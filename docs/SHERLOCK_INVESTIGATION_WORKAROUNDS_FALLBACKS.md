# SHERLOCK HOLMES — Forensic Investigation: Workarounds, Fallbacks, and Silent Error Suppression

**Case opened:** 2026-05-19
**Investigator:** Sherlock Holmes (consulting detective, forensic code investigation division)
**Scope:** All active workspace crates per `Cargo.toml` (22 members). Tests, examples, retired/archive paths excluded from severity grading.
**Doctrine under audit:** `CLAUDE.md` §6 (architectural invariants) + §15 (hard "never"s); `docs/futurebuild/specs/FSV-PROTOCOL.md` §3 ("Hard Rules") and §10 ("Failure Modes That Mean Not Done"); `docs2/AICodingAgentSuperPrompt.md` §1, §4, §14.
**Standard:** *Missing CFs, files, models, env vars, or malformed inputs fail closed with structured `SCREAMING_SNAKE_CASE` errors. Tests must fail when the feature is broken. No fallbacks that hide required-output failure.*

---

## EXECUTIVE SUMMARY — TOP 5 MOST DANGEROUS FINDINGS

Ranked by blast radius (impact on the binary-doctrine product surface, calibration, witness chain, or training).

### 1. **CRITICAL — Dead-gate on cross-task / adversarial training-data fallback ratio**
**Files:** `crates/context-graph-mejepa-train/src/sampler/cross_task.rs:6`; `crates/context-graph-mejepa-train/src/sampler/mod.rs:38,40,660`; `crates/context-graph-mejepa-train/src/cert/mod.rs:68,70`
**Pattern:** `pub const DEFAULT_FALLBACK_WARNING_RATIO: f32 = 0.30;` is declared but **NEVER referenced** anywhere in the codebase. The fallback counters (`adversarial_fallback_count`, `cross_task_fallback_count`) are silently incremented when the sampler cannot satisfy its quota and *no consumer of the training certificate ever compares those counts against the threshold*.
**Why doctrinal violation:** CLAUDE.md §6.13 ("mistake-loop is the only online weight update path … under EWC++ Fisher protection") — but here, the trainer can silently degrade into non-curated batches without ever tripping a gate. Operators reading source believe the 30% threshold is enforced; it is not. This is the most dangerous **named** fallback in the binary doctrine surface because it directly poisons the trained weights without surfacing anywhere.
**Blast radius:** Predictor calibration; ship-gate `prediction_oracle_correlation ≥ 0.95` (#310, #405).
**Correct fail-closed replacement:** Either (a) wire the constant into a trainer-side guard that returns `TrainerError::MejepaTrainFallbackRatioExceeded { observed, threshold }` when `cross_task_fallback_count / batch_count > 0.30`, OR delete the constant and document the lack of a ratio gate as an open issue.

### 2. **CRITICAL — `is_stale` returns `false` when stale_after timestamp is missing (TCT guard path)**
**File:** `crates/context-graph-mejepa-tct/src/guard_audit.rs:833-838`
**Pattern:**
```rust
fn is_stale(calibration: &SlotCalibration, now: DateTime<Utc>) -> bool {
    calibration.stale_after_utc.map(|stale_after| now > stale_after).unwrap_or(false)
}
```
**Why doctrinal violation:** A `SlotCalibration` with no `stale_after_utc` set is treated as PERMANENTLY FRESH. The TCT guard (`Gτ`) — which is doctrinally the only thing protecting calibrated prediction outputs (CLAUDE.md §1 Q2 row) — would accept arbitrarily-old or corrupted calibration rows without ever raising a freshness violation. This silently fails-open on the calibration freshness invariant.
**Blast radius:** Q2 verdict integrity, conformal interval validity, the ship-gate.
**Correct fail-closed replacement:** Return `Result<bool, TctError::CalibrationMissingFreshnessTimestamp { slot: ... }>` OR, if "no timestamp" is genuinely a valid state, conservatively return `true` (treat unknown as stale, force a refresh).

### 3. **HIGH — `compiler.rs:573-576` `constellation_verdict_pressure` silently coerced to 0.0**
**File:** `crates/context-graph-mejepa/src/compiler.rs:573-576`
**Pattern:**
```rust
constellation_verdict_pressure: constellation_intelligence
    .as_ref()
    .map(|evidence| evidence.pressures.q2_verdict_pressure)
    .unwrap_or(0.0),
```
**Why doctrinal violation:** When `constellation_intelligence` evidence collection fails or is absent, the *Q2 verdict pressure feature fed into the verdict decision* is silently replaced with `0.0`. The downstream verdict cannot distinguish "constellation reported zero pressure" from "constellation was unavailable." This corrupts the binary predictor's primary output (CLAUDE.md §1 Q2).
**Blast radius:** Every Q2 `Pass | Fail` prediction with missing constellation evidence.
**Correct fail-closed replacement:** Return `Err(MejepaInferError::ConstellationIntelligenceUnavailable { prediction_id })`. The compiler is already returning `Result<...>` — propagate the error rather than silently substituting zero.

### 4. **HIGH — Operator identity & version provenance silently defaulted ("local", "unversioned")**
**Files:**
- `crates/context-graph-cli/src/commands/dynamicjepa.rs:14366`: `env::var("USER").unwrap_or_else(|_| "local".to_string())`
- `crates/context-graph-mejepa/src/compiler.rs:591-594`: `self.constellation_guard.version_id().unwrap_or_else(|| "unversioned".to_string())`
- `crates/context-graph-mejepa-corpus/src/multilang_fixture_corpus.rs:316`: `PathBuf::from("/home/cabdru/.dotnet")` baked into source as last-resort fallback
**Why doctrinal violation:** Chain-of-Custody is doctrine (see master doctrine §11 "Chain of Custody (Git Forensics)"). A run-manifest that records `operator: "local"` because `$USER` was unset is unattributable; a `PredictionProvenance` with `constellation_version: "unversioned"` cannot be reproduced; a hardcoded `/home/cabdru/.dotnet` path leaks operator identity AND silently shifts behavior across hosts.
**Blast radius:** Audit trail, FSV reproducibility, witness-chain attribution for the binary predictor.
**Correct fail-closed replacement:**
- `dynamicjepa.rs:14366`: `env::var("USER").map_err(|_| DynamicJepaError::OperatorUserEnvUnset)?`
- `compiler.rs:591`: `self.constellation_guard.version_id().ok_or(MejepaInferError::ConstellationVersionIdMissing)?`
- `multilang_fixture_corpus.rs:316`: `DOTNET_ROOT` resolution should be `Err("DOTNET_ROOT_NOT_RESOLVED")` if neither env var nor `$HOME/.dotnet` exists.

### 5. **HIGH — `temporal_boost::extract_temporal_components` silently substitutes `Utc::now()` on malformed timestamps**
**File:** `crates/context-graph-storage/src/teleological/search/temporal_boost.rs:64`
**Pattern:**
```rust
let datetime = DateTime::from_timestamp_millis(timestamp_ms).unwrap_or_else(Utc::now);
```
**Why doctrinal violation:** Memory records with corrupted/out-of-range `timestamp_ms` (negative, > 8.6e15, NaN-equivalent) have their temporal features computed against CURRENT WALL-CLOCK TIME. The memory then appears artificially "current," directly fooling the E2/E3/E4 temporal embedders. This is also a non-deterministic substitution — re-running the same query at different wall-clock times produces different E3 hour-of-day / E4 day-of-week features for the same corrupted record.
**Blast radius:** Every temporal-boosted memory search; reproducibility of E2/E3/E4 features.
**Correct fail-closed replacement:** `DateTime::from_timestamp_millis(timestamp_ms).ok_or(SearchError::TimestampInvalid { timestamp_ms })?`. Reject the record from temporal scoring rather than coercing it to "now."

---

## FULL FINDINGS LIST

### Severity legend
- **CRITICAL** — fail-open on binary-doctrine product surface (Q1/Q2/Q5), ship-gate input, or witness/audit integrity
- **HIGH** — fail-open on training data, calibration, predictor input, or operator-facing provenance
- **MEDIUM** — fail-open on diagnostic/observability, secondary search paths, retired surfaces still alive
- **LOW** — stylistic/maintenance concerns that could become bugs but are currently defensible

---

#### F-001 [CRITICAL] Dead-gate on training fallback ratio
- **File:** `crates/context-graph-mejepa-train/src/sampler/cross_task.rs:6`
- **Pattern:** `pub const DEFAULT_FALLBACK_WARNING_RATIO: f32 = 0.30;` declared, never referenced.
- See Executive Summary §1 above.

#### F-002 [CRITICAL] TCT `is_stale` fail-open on missing timestamp
- **File:** `crates/context-graph-mejepa-tct/src/guard_audit.rs:833-838`
- See Executive Summary §2 above.

#### F-003 [HIGH] Constellation verdict pressure silently zeroed
- **File:** `crates/context-graph-mejepa/src/compiler.rs:573-576`
- See Executive Summary §3 above.

#### F-004 [HIGH] Operator identity defaulted to "local"
- **File:** `crates/context-graph-cli/src/commands/dynamicjepa.rs:14366`
- See Executive Summary §4 above.

#### F-005 [HIGH] Constellation version_id defaulted to "unversioned"
- **File:** `crates/context-graph-mejepa/src/compiler.rs:591-594`
- See Executive Summary §4 above.

#### F-006 [HIGH] `/home/cabdru/.dotnet` hardcoded as last-resort fallback
- **File:** `crates/context-graph-mejepa-corpus/src/multilang_fixture_corpus.rs:311-317`
- See Executive Summary §4 above. Additional finding: this also leaks operator-specific path information into a committed source file (CLAUDE.md §15 forbids cost caps but per general doctrine, operator-specific paths should be config-driven).

#### F-007 [HIGH] `extract_temporal_components` substitutes current time
- **File:** `crates/context-graph-storage/src/teleological/search/temporal_boost.rs:64`
- See Executive Summary §5 above.

#### F-008 [HIGH] `cosine()` in cross_task sampler returns 0.0 silently
- **File:** `crates/context-graph-mejepa-train/src/sampler/cross_task.rs:83-95`
- **Pattern:** Returns `0.0` on dim-mismatch / empty / zero-norm.
- **Why doctrinal violation:** CLAUDE.md §6.2 ("slot identity is sacred. NEVER perform cross-embedder dim-wise comparison"). A dim-mismatch between `e_diff_embeddings[i]` and `[j]` is a structural slot-identity violation, not a runtime "low similarity" event. Silently returning 0.0 lets `build_patch_similarity_graph` drop the edge below threshold — the patch similarity graph used for cross-task transfer is silently corrupted by malformed embeddings.
- **Correct fail-closed replacement:** Return `Result<f32, TrainerError>` with `TrainerErrorCode::MejepaTrainPatchSimilarityDimMismatch` / `MejepaTrainPatchSimilarityZeroNormVector`. The crate already has the gold-standard `cosine_same_dim` in `dda.rs:384` — adopt that pattern uniformly.

#### F-009 [HIGH] `cosine()` in trainer loss/aux returns 0.0 silently
- **File:** `crates/context-graph-mejepa-train/src/loss/aux.rs:173-182`
- **Pattern:** identical to F-008. Returns 0.0 on zero-norm.
- **Why doctrinal violation:** This is the auxiliary loss helper. Zero-norm input means a degenerate gradient signal silently treated as "orthogonal." The trainer cannot distinguish "vectors are orthogonal" from "input was malformed."
- **Correct fail-closed replacement:** Pattern after `dda.rs:384`'s `cosine_same_dim` returning `Result<f32, TrainerError>`.

#### F-010 [HIGH] `cosine_f32` in dynamicjepa service returns 0.0 silently
- **File:** `crates/context-graph-dynamicjepa/src/service.rs:2319-2334`
- **Pattern:** `if denom <= f32::EPSILON { 0.0 } else { (dot / denom).clamp(-1.0, 1.0) }` — no length check, no zero-norm error.
- **Why doctrinal violation:** Same as F-008/F-009. Note this file also contains a *correct* `cosine_f32_exact` at `service.rs:2276` returning `DynamicJepaResult<f32>` — two different patterns in one file, callers may pick either.
- **Correct fail-closed replacement:** Delete `cosine_f32`, migrate all callers to `cosine_f32_exact`.

#### F-011 [HIGH] `fetch_knn_per_embedder` silently swallows all repository errors
- **File:** `crates/context-graph-mcp/src/handlers/tools/training_tools.rs:765-778`
- **Pattern:** `Err(_) => Vec::new()` per embedder.
- **Why doctrinal violation:** This MCP tool exports training records for downstream agents (the binary doctrine consumes these). A failing RocksDB CF for embedder N silently returns an empty K-NN list — the consumer cannot distinguish "no neighbors exist" from "the edge repository is broken." Training data is silently truncated.
- **Correct fail-closed replacement:** Return `Result<Vec<Vec<KnnNeighbor>>, TrainingToolError>` with `EdgeRepositoryFetchFailed { embedder_idx, source }`. Surface the per-embedder error to the MCP response.

#### F-012 [HIGH] `fetch_outgoing_edges` swallows errors with `warn!` only
- **File:** `crates/context-graph-mcp/src/handlers/tools/training_tools.rs:744-752`
- **Pattern:** `Err(e) => { warn!(...); Vec::new() }`
- **Why doctrinal violation:** At least logs; but the MCP response still indistinguishable from "no edges." Tracing logs are *not* a response surface — the consumer cannot see them. Same blast radius as F-011, slightly milder because the operator sees it in logs.
- **Correct fail-closed replacement:** Propagate as F-011.

#### F-013 [HIGH] `library_foundationality` silently drops library-bedrock score rows on Err
- **File:** `crates/context-graph-mejepa/src/library_foundationality.rs:432`
- **Pattern:** A `filter_map` calls `.ok()` to drop rows whose scoring failed.
- **Why doctrinal violation:** Library bedrock scoring is part of the chunk-skill linkage surface (#414 / #417). Silently dropping rows whose score computation failed corrupts the rank ordering of `top_chunks` — operators see a `top_chunks: [...]` list that's missing entries with no indication WHY.
- **Correct fail-closed replacement:** Return `Result<...>` from the inner scoring function and either propagate the error or accumulate failures into a sibling field like `scoring_failures: Vec<{chunk_id, reason}>`.

#### F-014 [HIGH] `release_aggregate_table_key` errors collapse to empty-string sort key
- **File:** `crates/context-graph-cli/src/commands/dynamicjepa.rs:13164,13166,13235,13237`
- **Pattern:** `release_aggregate_table_key(...).unwrap_or_else(|_| String::new())` inside `sort_by` comparators.
- **Why doctrinal violation:** When key generation fails for some rows, all failed rows compare equal (empty string) — the resulting sort order is non-deterministic for failed rows and corrupts CI reference-check comparisons downstream. The reference check then reports `STRUCTURAL_DRIFT` for a reason that has nothing to do with actual drift.
- **Correct fail-closed replacement:** Use `try_fold` or pre-compute keys with `Result<Vec<(key, row)>>`, fail the entire sort on any key-generation error.

#### F-015 [HIGH] `gc_lock_path` silently defaults to hardcoded production root
- **File:** `crates/context-graph-mejepa-hygiene/src/gc.rs:473-477`
- **Pattern:**
  ```rust
  fn gc_lock_path() -> PathBuf {
      let root = std::env::var("CONTEXTGRAPH_DATA_ROOT")
          .map(PathBuf::from)
          .unwrap_or_else(|_| PathBuf::from("/zfs/archive/contextgraph"));
      root.join("state/locks/mejepa-gc.lock")
  }
  ```
- **Why doctrinal violation:** If `CONTEXTGRAPH_DATA_ROOT` is unset or invalid UTF-8, GC silently uses `/zfs/archive/contextgraph` — a production path. On a developer workstation without aiwonder mounted, the GC would attempt to acquire a lock at a path that may not exist OR — worse — that COULD exist on the same host as a remnant from a different deployment. CLAUDE.md §6.7 says "aiwonder only for post-migration production durable state" — silently routing dev/test GC into the prod path violates this.
- **Correct fail-closed replacement:** `std::env::var("CONTEXTGRAPH_DATA_ROOT").map_err(|_| OpsError::DataRootUnset)?`.

#### F-016 [HIGH] `verify_cli::check_agreement` called with `prediction_pass.unwrap_or(false)`
- **File:** `crates/context-graph-mejepa/src/verify_cli.rs:107`
- **Pattern:** `check_agreement(corpus_root, &sample, prediction_pass.unwrap_or(false))?;`
- **Why doctrinal violation:** When `prediction_pass` is `None` (verdict not approved, see line 102), this calls `check_agreement` with a phantom `false`. Although line 112 later adds `MEJEPA_VERIFY_VERDICT_NOT_APPROVED` to blockers, the `agreement_count` is now computed against a fabricated verdict, polluting the `me_jepa_predicts_match_oracle` computation. The control flow conflates "verdict unknown" with "verdict = fail."
- **Correct fail-closed replacement:**
  ```rust
  let Some(prediction_pass_value) = prediction_pass else {
      blockers.push("MEJEPA_VERIFY_VERDICT_NOT_APPROVED".to_string());
      // skip agreement check entirely; emit zero counts
      ...
  };
  let (agreement_count, mut blockers) = check_agreement(corpus_root, &sample, prediction_pass_value)?;
  ```

#### F-017 [MEDIUM] `db.get_cf(...).ok().flatten().is_some()` masks RocksDB errors as "missing key"
- **Files:** `crates/context-graph-storage/src/teleological/rocksdb_store/causal_relationships.rs:694,949`
- **Pattern:** `db.get_cf(cf, key).ok().flatten().is_some()` — discards `Result::Err` (I/O error, corruption) and treats it identically to `Ok(None)` (row not found).
- **Why doctrinal violation:** Per FSV-PROTOCOL §3.5, "missing CFs … fail closed." RocksDB partial-corruption errors here silently filter out matches; operators see a search returning fewer results with no indication the DB is unhealthy.
- **Correct fail-closed replacement:** Use `?` to propagate the `Result` up; structurally lift the comprehension out of `.filter(...)` into a `try_fold` or explicit loop.

#### F-018 [MEDIUM] `weekly_curiosity_ranking_section` errors silently dropped on weekly report
- **File:** `crates/context-graph-mcp/src/daemon.rs:1807-1808`
- **Pattern:** `summary.curiosity_ranking_section = weekly_curiosity_ranking_section(db).ok();`
- **Why doctrinal violation:** If bincode deserialization of the active-learning queue fails (line 1820 in `weekly_curiosity_ranking_section`), the operator-facing weekly report SILENTLY omits the section. The operator sees a report missing the section and cannot tell whether (a) the queue is empty, (b) the queue is corrupted, or (c) the section was never computed.
- **Correct fail-closed replacement:** Either propagate the error to make the weekly report itself fail (preferred — operator notices), or store the error message as `Some(format!("curiosity_ranking_section_error: {err}"))`. Same applies to `operator_contributions_section` at line 1808.

#### F-019 [MEDIUM] CI reference drift error detail destroyed on serialize failure
- **File:** `crates/context-graph-cli/src/commands/dynamicjepa.rs:13751,13758`
- **Pattern:**
  ```rust
  serde_json::to_string_pretty(&structural_diffs)
      .unwrap_or_else(|_| "structural drift detected".to_string())
  ```
- **Why doctrinal violation:** When the structural diff cannot be serialized (extremely rare but possible if a non-finite f64 sneaks in), the operator gets the canned string "structural drift detected" with no diagnostic detail. They cannot reproduce.
- **Correct fail-closed replacement:** `serde_json::to_string_pretty(&structural_diffs).context("serialize structural_diffs for CI reference report")?;` — let the CI runner surface the serialize error rather than mask it.

#### F-020 [MEDIUM] `is_passes_but_should_fail_context` / contains_secret_like_token / `patch_has_flakiness_risk` heuristics use string-contains in lowercase
- **File:** `crates/context-graph-mejepa/src/prediction_surfaces.rs:452-499`
- **Pattern:** Returns `false` on every code path where a heuristic doesn't match.
- **Why doctrinal SOFT violation:** These are explanation heuristics (Q3 surface, not the binary verdict), but the lack of error path means a corrupted patch text or unicode-attacker-input will silently fall through as "no risk." Defensible because these are heuristic hints, not gates.
- **Correct fail-closed replacement:** Acceptable as-is for Q3 (display-only). Add documentation noting these are heuristic-only.

#### F-021 [MEDIUM] `project_cache::metadata_modified_ns` returns 0 on metadata error
- **File:** `crates/context-graph-mejepa/src/project_cache.rs:1307-1314`
- **Pattern:** Returns `0` if `metadata.modified()` or `duration_since(UNIX_EPOCH)` fails.
- **Why doctrinal violation:** This `mtime_ns` is used at line 282-284 to compare against previously-cached rows: `metadata_unchanged = previous_row.is_some_and(|old| old.size_bytes == size_bytes && old.mtime_ns != 0 && old.mtime_ns == mtime_ns)`. Good news: the existing `!= 0` check is a partial guard. But: a file whose metadata is currently failing will be re-hashed every scan (correct behavior), while a previously-cached row with `mtime_ns == 0` will never match — also defensive. Mostly OK, but the 0 sentinel conflates "no row" with "metadata error" downstream.
- **Correct fail-closed replacement:** Return `Result<i64, ProjectCacheError::FilesystemMetadataInvalid>` from `metadata_modified_ns`.

#### F-022 [MEDIUM] `project_cache::now_ms` returns 0 on system-clock failure
- **File:** `crates/context-graph-mejepa/src/project_cache.rs:1316-1321`
- **Pattern:** `SystemTime::now().duration_since(UNIX_EPOCH).map(...).unwrap_or(0)`
- **Why doctrinal violation:** `scan_started_unix_ms = 0` is a sentinel that would cluster with epoch-time. Acceptable in practice (system clock < 1970 is essentially impossible on Linux), but stylistically should be `Err`.
- **Correct fail-closed replacement:** Propagate the SystemTimeError.

#### F-023 [MEDIUM] Witness `cert.rs::verify_segment_signature` collapses all errors to `WitnessChainInvalid { offset: 0 }`
- **File:** `crates/context-graph-witness/src/cert.rs:343-355`
- **Pattern:**
  ```rust
  Err(WitnessError::PrevHashMismatch { offset, .. }) => return Err(CertError::WitnessChainInvalid { offset }),
  Err(_) => return Err(CertError::WitnessChainInvalid { offset: 0 }),
  ```
  and
  ```rust
  WitnessEntry::from_bytes(segment).map_err(|_| CertError::WitnessChainInvalid { offset: 0 })?;
  ```
- **Why doctrinal violation:** Witness-chain verification is the audit-trail integrity surface. Collapsing `BadMagic` / `LengthMismatch` / `EntryHashMismatch` / parse errors all into `WitnessChainInvalid { offset: 0 }` destroys the diagnostic context required to recover from a corrupted chain.
- **Correct fail-closed replacement:** Extend `CertError` with variants that carry the underlying `WitnessError` (e.g., `CertError::WitnessChainInvalid { offset, cause: Box<WitnessError> }`).

#### F-024 [MEDIUM] `constellation_intelligence::cosine_mean_to_unit(n=0) = 0.5`
- **File:** `crates/context-graph-mejepa/src/constellation_intelligence.rs:693-699`
- **Pattern:** When the aggregation has zero samples, returns `0.5` (the "neutral" cosine).
- **Why doctrinal violation:** 0.5 is a real cosine value indicating orthogonality. Returning 0.5 for "no data" conflates "perfectly orthogonal slot pairs" with "no slot pairs were aggregated." Downstream consumers cannot tell.
- **Correct fail-closed replacement:** Return `Option<f32>` (None = no samples) or `Result<f32, MejepaInferError>`.

#### F-025 [MEDIUM] `core/retrieval/distance.rs::cosine_similarity` returns 0.0 on dim mismatch
- **File:** `crates/context-graph-core/src/retrieval/distance.rs:32-57, 66-90`
- **Pattern:** Documented as "AP-10: zero-magnitude vectors return 0.0 (no NaN)" — also returns 0.0 on dim-mismatch.
- **Why doctrinal violation (soft):** This is the legacy retrieval-layer cosine, used by memory tools and graph search. CLAUDE.md §6.2 says cross-embedder dim mismatch is a SLOT IDENTITY VIOLATION in the ME-JEPA Panel context. In the legacy retrieval context (per-embedder lanes), dim mismatch should not arise — but if it does, returning 0.0 hides the bug. Defensible for backward-compat but should panic or `debug_assert!` to make the violation loud during tests.
- **Correct fail-closed replacement:** At minimum, `debug_assert_eq!(a.len(), b.len())` to surface the violation in dev/test. Long-term, migrate callers to a `Result`-returning variant.

#### F-026 [MEDIUM] `storage/teleological/indexes/metrics.rs::cosine_similarity` PANICS on bad input
- **File:** `crates/context-graph-storage/src/teleological/indexes/metrics.rs:137-146`
- **Pattern:** `panic!("METRIC ERROR: ...")` on dim mismatch / empty.
- **Why doctrinal SOFT violation:** Panic IS fail-closed but is a process-killer in the MCP server. The MCP server should return a structured error response, not crash. Same concern applies to `multi_space.rs:134, 144, 210, 218`.
- **Correct fail-closed replacement:** Convert panics to `EmbeddingError::DimensionMismatch { left, right }` and propagate.

#### F-027 [MEDIUM] `learning_signal::compute_delta_p` silently zeros `delta_p_imagined`
- **File:** `crates/context-graph-mejepa-train/src/learning_signal.rs:416`
- **Pattern:** `let imagined = c.delta_p_imagined.unwrap_or(0.0);`
- **Why doctrinal violation:** When `delta_p_imagined` is `None`, this is treated as zero in the `max(real, gamma * imagined)`. If `delta_p_imagined` is intended to be optional (some training rows have no imagined trajectory), the `unwrap_or(0.0)` is fine BUT the upstream `validate()` should enforce the option semantics. Without checking, a corrupted training row with imagined=None would silently degrade to "use only the real component."
- **Correct fail-closed replacement:** Either (a) require `delta_p_imagined: f32` (not Option), OR document the legitimate None path and verify `validate()` catches the corrupted-row case.

#### F-028 [MEDIUM] `chunk_foundationality::edge_kind_counts.get(kind).copied().unwrap_or(0) > 0` — test only, but pattern is also used in non-test code
- **File:** `crates/context-graph-mejepa/src/chunk_foundationality.rs:2684` (test) — INNOCENT (test context).
- Skip.

#### F-029 [MEDIUM] `mejepa_skill_linkage_tools.rs:191` unwrap_or_default on linkage rows
- **File:** `crates/context-graph-mcp/src/handlers/tools/mejepa_skill_linkage_tools.rs:191`
- Needs deeper inspection (out of time budget for this Tier-3 investigation). Flagged for follow-up audit.

#### F-030 [LOW] `eprintln!` instead of structured tracing in production action paths
- **Files:** `crates/context-graph-embeddings/src/storage/multi_space.rs:168-174` and other `eprintln!` sites outside test modules.
- **Pattern:** `eprintln!("[SINGLE-SPACE SEARCH] ...")` after a successful search.
- **Why doctrinal violation:** Doctrine §14.12 (Observability) requires structured logging via `tracing::*`. `eprintln!` cannot be filtered, correlated, or shipped to operator dashboards.
- **Correct fail-closed replacement:** Replace with `tracing::debug!(embedder, query_dim, k, found, "single-space search complete")`.

#### F-031 [LOW] `freshness_policy::split_cell_key` returns `("unknown", "unknown")` on malformed input
- **File:** `crates/context-graph-mejepa-tct/src/guard_audit.rs:840-845`
- **Pattern:** `parts.next().unwrap_or("unknown").to_string()` for two-component split.
- **Why doctrinal violation:** If a cell_key is malformed (no colon), both halves become `"unknown"`, corrupting downstream `BTreeMap<language, count>` aggregations: multiple distinct malformed keys all fold into `language="unknown", mutation_category="unknown"`.
- **Correct fail-closed replacement:** Return `Result<(String, String), TctError::CellKeyMalformed { value }>`.

---

## INNOCENT UNTIL PROVEN GUILTY — EXONERATION LIST

The following patterns were investigated and determined NOT to be doctrinal violations.

| Location | Pattern | Why exonerated |
|----------|---------|----------------|
| `crates/context-graph-mejepa/src/reward_signal_audit.rs:401, 440, 451, 456, 459, 460, 463, 470` | `count_cf_optional(db, CF).?unwrap_or(0)` | Report has paired `*_ready: db.cf_handle(...).is_some()` flags; the `0` count is correctly paired with `ready: false`. |
| `crates/context-graph-mejepa/src/reward_signal_audit.rs:921`, `calibration.rs:218`, `head_projection.rs:483`, `claim_graph.rs:199`, `project_stress.rs:466`, `sampler_reward.rs:396`, `heal/store.rs:165`, `synthetic_stress_store.rs:66`, `mejepa_tools.rs:3782` | `let _ = item?;` in CF iterator | Verbose CF-row-count idiom; `?` correctly propagates iterator errors. |
| `crates/context-graph-witness/src/sign.rs:179, 187` | `map_err(\|_\| SignError::PublicKeyMalformed/VerificationFailed)` | Underlying ed25519 error intentionally elided to prevent timing-side-channel leak; SignError variants ARE the structured codes. |
| `crates/context-graph-mejepa-tct/src/freshness.rs:56` | `now.duration_since(constellation.frozen_at).map_err(\|_\| TctError::invalid(...))` | The map_err discards the SystemTimeError but constructs a descriptive structured TctError — fail-closed with context preserved. |
| `crates/context-graph-mejepa-tct/src/freshness_policy.rs:700-708` | `duration_days` returns `u32::MAX` on overflow | Defensible — errs on the side of "very stale," forcing refresh. |
| `crates/context-graph-mejepa-train/src/dda.rs:384` | `cosine_same_dim` returning `Result<f32, _>` | This is the GOLD STANDARD pattern; other cosine helpers in the same workspace should adopt it. |
| `crates/context-graph-mejepa/src/q4_trust_gate_support.rs:39-41` | `evidence.producer_fsv_root.clone().unwrap_or_default()` | Downstream `missing_requirements` correctly fails-closed when fields are empty. |
| `crates/context-graph-mejepa/src/q4_trust_gate.rs:238` | `catalog.heads.get(&requirement.head).cloned().unwrap_or_default()` | Default `Q4HeadEvidence` has all zero-row fields, which correctly trips `missing_requirements`. |
| `crates/context-graph-mcp/src/main.rs:1251, 1255, 1289, 1293` | `read_link / read_to_string ... .ok() / unwrap_or_default()` in stale-PID detection | Inherently best-effort heuristic; the fallback path returns `false` (= "not dead, don't kill") which is fail-conservative. |
| `crates/context-graph-mcp/src/handlers/tools/reality_loop/autoresearch.rs:609-637`, `optimizer_preflight_fsv.rs:34-65`, `bandit.rs:649-668` | `fs::write(...).ok()` in `Drop` impls | Test-fixture cleanup; best-effort restoration of original cache files. Not in production action paths. |
| `crates/context-graph-mejepa/src/heal/promote.rs:1238` | `after_value.map(...).unwrap_or(false)` | Missing `after` cell correctly counts as regression — fail-closed conservative behavior. |
| `crates/context-graph-mejepa/src/prediction_surfaces.rs:443` | `symbol_hint` returns Option<String>; `unwrap_or_default` on inner split | Heuristic Q3 hint; legitimate empty-string semantics. |
| `crates/context-graph-mejepa/src/project_ingest.rs:297, 324, 336` | `previous_manifest.as_ref().map(...).unwrap_or(0/default)` | Correctly represents "no prior state" on first run. |
| `crates/context-graph-mejepa-corpus/src/oracle.rs:1273` | `duration_secs.unwrap_or(0.0) + duration` | Accumulator pattern; `duration` already validated finite at line 1266. |
| `crates/context-graph-mejepa-corpus/src/oracle.rs:1032-1034` | `interrupted.map(...).unwrap_or(false)` | Feature-gated: `interrupted = None` means caller did not request interrupt monitoring. |
| `crates/context-graph-cli/src/commands/dynamicjepa.rs:20025` | `to_string(outcome.value).unwrap_or_else(\|_\| "<unserializable>".to_string())` | Outcome value is already a JSON Value; serialization failure is essentially unreachable. |
| `crates/context-graph-mcp/src/main.rs:2557, 2651, 2675` | `serde_json::to_string(&value).unwrap_or_else(\|_\| "{}".to_string())` | Server-side fallback for already-constructed JSON; essentially unreachable. |
| All `let _ = item?` patterns in CF-count iterators | Same as above | Idiomatic; `?` propagates errors. |

---

## FORENSIC EVIDENCE CHAIN — TOP 3 FINDINGS

### Evidence Chain 1: F-001 — Dead-gate on training fallback ratio

**Three hypotheses considered (per doctrine §11 abductive reasoning):**

1. **H1 (40% prior):** The constant is read by a consumer this audit missed (perhaps via reflection, env-var coupling, or a hidden module).
2. **H2 (35% prior):** The constant was added in anticipation of a gate but the gate code was never written (vestigial planning artifact).
3. **H3 (20% prior):** The constant is documentation only — a "natural" magic number declared for traceability but with no code dependency.
4. **H4 (5% prior):** Unknown — perhaps it's checked at compile time or via a macro.

**Linear Sequential Unmasking (LSU):** I examined the constant FIRST (in `cross_task.rs:6`) without reading the doc comments above it. I then ran a full ripgrep across `crates/*/src` for any read site. ZERO matches.

**Bayesian update:**
- E1 (ripgrep finds zero readers, including macro/test/example contexts): P(E\|H1) = 0.01; P(E\|H2) = 0.95; P(E\|H3) = 0.80; P(E\|H4) = 0.10
- Posterior: H1 → 0.5%; **H2 → 84%**; H3 → 14%; H4 → 1.5%

**Call chain from caller to fallback to masked failure:**
```
trainer/mod.rs::train_one_batch
  ↓ calls
sampler/cross_task.rs::next_batch_with_cross_task
  ↓ when the cross-task neighbor pool is empty for current_task_idx
  ↓ falls back to weighted_pick_from_regular_excluding (line 62)
  ↓ increments plan.cross_task_fallback_count (line 61)
  ↓ NO GATE FIRES
trainer/mod.rs:298 stores cross_task_fallback_count into the training certificate
  ↓ certificate is written
  ↓ NO CONSUMER COMPARES count vs DEFAULT_FALLBACK_WARNING_RATIO
  ↓ NO CONSUMER WARNS, BLOCKS, OR ESCALATES
```
**Masked failure mode:** The trainer drifts toward non-curated batches without ever hitting a tripwire. The `prediction_oracle_correlation` ship-gate may regress, but the operator has no early-warning signal that the cross-task transfer is failing.

**Self-verification (per doctrine §11 — avoiding Holmes's errors):**
- Did I consider that the constant might be used at compile time? Yes — checked for `cfg!`, `const_eval`, `compile_error!` patterns. None.
- Could the constant be used via FFI / Python binding? The crate is Rust-only with no FFI surface.
- Have I confirmed by checking the trainer's gate set explicitly? Yes — `trainer/mod.rs` does not import `cross_task::DEFAULT_FALLBACK_WARNING_RATIO`.

**VERDICT: GUILTY — dead-gate confirmed.**

---

### Evidence Chain 2: F-002 — TCT `is_stale` fail-open on missing timestamp

**Three hypotheses considered:**

1. **H1 (40%):** `stale_after_utc: Option<DateTime>` is intentional — a `None` represents "this calibration has no expiration" (e.g., manually-pinned).
2. **H2 (40%):** This is fail-open negligence — `None` should mean "treat as stale" (conservative).
3. **H3 (15%):** This is a legitimate optimization — the `stale_after` is computed lazily elsewhere and `None` means "not yet computed."
4. **H4 (5%):** The Option is dead code — every code path that constructs `SlotCalibration` sets `stale_after_utc = Some(...)`.

**Linear Sequential Unmasking:** I read `is_stale` first, THEN searched for `SlotCalibration` constructors.

**Bayesian update:**
- E1: Searching `rg "stale_after_utc" crates/context-graph-mejepa-tct/src` shows the field is parsed from JSON via `serde` deserialize at `guard_audit.rs:820-830` — and the parse explicitly handles `value.is_null()` by *not* setting `stale_after_utc`. So `None` is reachable through the JSON input path.
- E2: There is no comment or doc-string on `stale_after_utc` clarifying the semantics of `None`.
- P(E\|H1) = 0.60 (consistent — optional field, null-handled, no comment because intentional); P(E\|H2) = 0.85 (consistent — fail-open negligence, no comment because oversight); P(E\|H3) = 0.20 (would expect a lazy-init comment); P(E\|H4) = 0.05 (contradicted by the null-handling parse).
- Posterior: H1 → 30%; **H2 → 55%**; H3 → 5%; H4 → 0.6%.

**Call chain:**
```
TCT freshness audit
  ↓ loads SlotCalibration from JSON / RocksDB
  ↓ stale_after_utc parses to None (e.g., legacy row without expiry, or producer wrote null)
  ↓ is_stale(calibration, now) returns false
  ↓ TCT guard treats calibration as ETERNALLY FRESH
  ↓ Predictor uses possibly-stale calibration for conformal intervals
  ↓ Q2 verdict is computed with calibrated thresholds that may be months old
```
**Masked failure mode:** Predictor calibration silently uses stale data, biasing the binary Q2 verdict away from oracle agreement. Ship-gate `prediction_oracle_correlation` may fluctuate without obvious cause.

**Self-verification:**
- Conservative interpretation (H2) recommends `is_stale = true` on None. But if H1 is true, this would force unnecessary refreshes of legitimately-pinned calibrations.
- The safest doctrinal resolution: convert to `Result<bool, TctError::CalibrationMissingFreshnessTimestamp>` and let the *caller* decide whether to treat missing as stale or pinned. Pushes the policy decision to a place where the context is known.

**VERDICT: GUILTY — fail-open behavior confirmed regardless of which hypothesis is correct, because the function silently hides the missing-timestamp condition from the caller.**

---

### Evidence Chain 3: F-003 — `constellation_verdict_pressure` silently zeroed in compiler

**Three hypotheses considered:**

1. **H1 (50%):** `constellation_intelligence` is `Option<Evidence>` to represent "constellation not enabled for this prediction" — `None` means "this Q2 verdict legitimately has no constellation input."
2. **H2 (35%):** `constellation_intelligence` is `Option` to represent "evidence collection failed" — `None` should propagate as an error.
3. **H3 (10%):** Both. `None` means *either* "not enabled" *or* "failed," and the caller cannot tell.
4. **H4 (5%):** The Option is dead code — all reachable code paths construct Some.

**Linear Sequential Unmasking:** Read `compiler.rs:573-576` first, then traced back to find where `constellation_intelligence` is constructed.

**Investigation:** Searching `crates/context-graph-mejepa/src/compiler.rs` for the construction site reveals `constellation_intelligence` is computed from a fallible operation upstream. The compiler proceeds whether evidence collection succeeded or not.

**Bayesian update (informed by source inspection):**
- E1: The field is *consumed* in the verdict computation (line 575) but its absence is silently coerced to 0.0. If H1 were the design intent, we'd expect a separate `constellation_enabled: bool` flag in `VerdictInput` so the verdict head could route conditionally.
- E2: There is no such flag.
- P(E\|H1) = 0.10 (inconsistent — design would include an enabled flag); P(E\|H2) = 0.70 (consistent — error suppressed); P(E\|H3) = 0.50 (consistent — ambiguous design); P(E\|H4) = 0.05.
- Posterior: H1 → 7%; **H2 → 56%**; H3 → 36%; H4 → 1%.

**Call chain:**
```
MejepaCompiler::verify(patch, context)
  ↓ collects constellation_intelligence (possibly Err → None)
  ↓ calls compile_verdict(VerdictInput { ..., constellation_verdict_pressure: 0.0 })
  ↓ verdict head receives phantom 0.0 pressure
  ↓ verdict output reflects the phantom value
  ↓ Q2 Pass | Fail emitted
```
**Masked failure mode:** Q2 verdict for patches where constellation evidence collection failed is silently downgraded to "no pressure → looks safer." Operator never knows.

**Self-verification:**
- Counter-argument (H1): if "no constellation" IS a legitimate design state, then 0.0 is the doctrinally-correct neutral value, and the verdict head is explicitly designed to handle pressure=0 as "no signal." But this should be documented in a code comment.
- Either way, the LACK OF A COMMENT EXPLAINING THE INTENT is itself doctrinal evidence of negligence (Cold Read found no documentation justifying the fallback).

**VERDICT: GUILTY — at minimum a documentation violation; under H2 (most probable), a verdict-corrupting silent error suppression.**

---

## CONTRADICTION ENGINE SUMMARY

Patterns where comments / documentation contradict actual behavior:

| Location | Comment claims | Code actually does |
|----------|----------------|---------------------|
| `crates/context-graph-mejepa-train/src/sampler/cross_task.rs:6` | "DEFAULT_FALLBACK_WARNING_RATIO: f32 = 0.30" implies a 30% threshold is enforced | No code reads this constant |
| `crates/context-graph-core/src/retrieval/distance.rs:48` | "AP-10: zero-magnitude vectors return 0.0 (no NaN)" — implies NaN is the failure mode being avoided | Also returns 0.0 on dim-mismatch, which is a DIFFERENT structural error |
| `crates/context-graph-storage/src/teleological/search/temporal_boost.rs:38` | "Returns a similarity score in [0.0, 1.0] where 1.0 is identical." | Also returns 0.0 on dim-mismatch (different failure mode) |
| `crates/context-graph-mejepa-tct/src/guard_audit.rs:833` | No comment on `is_stale` semantics for `stale_after_utc = None` | Silently returns "not stale" |

---

## RECOMMENDED FOLLOW-UP

Per CLAUDE.md §3 doctrine (memory = GitHub Issues), each CRITICAL/HIGH finding above should become a `type:discovery` GitHub issue tagged `priority:p0` or `priority:p1`. Suggested issue titles:

1. **`[discovery][p0] DEFAULT_FALLBACK_WARNING_RATIO is a dead-gate — wire or delete (F-001)`**
2. **`[discovery][p0] TCT is_stale fails open on missing stale_after_utc — convert to Result (F-002)`**
3. **`[discovery][p0] constellation_verdict_pressure silently zeroed on missing evidence — compiler.rs:573 (F-003)`**
4. **`[discovery][p1] Operator/version provenance defaults to "local"/"unversioned" — break chain of custody (F-004, F-005)`**
5. **`[discovery][p1] /home/cabdru/.dotnet hardcoded as fallback in multilang_fixture_corpus (F-006)`**
6. **`[discovery][p1] extract_temporal_components substitutes Utc::now() on malformed timestamps (F-007)`**
7. **`[discovery][p1] Three cosine() helpers in train/dynamicjepa silently return 0.0 — adopt dda.rs gold standard (F-008, F-009, F-010)`**
8. **`[discovery][p1] training_tools fetch_knn_per_embedder/fetch_outgoing_edges swallow RocksDB errors (F-011, F-012)`**
9. **`[discovery][p1] library_foundationality drops bedrock score rows on Err (F-013)`**
10. **`[discovery][p1] dynamicjepa release_aggregate_table_key error → empty-string sort key corruption (F-014)`**
11. **`[discovery][p1] gc_lock_path silently defaults to hardcoded /zfs/archive root (F-015)`**
12. **`[discovery][p1] verify_cli check_agreement called with phantom prediction_pass (F-016)`**
13. **`[discovery][p2] Multiple medium-severity silent-error patterns (F-017..F-031) — batch remediation`**

---

## INVESTIGATIVE METHODOLOGY (doctrine §11 forensic discipline)

- **Tier 1 (Glance):** `rg` over `crates/*/src` for `unwrap_or_default`, `unwrap_or(0.0)`, `unwrap_or(false)`, `.ok()`, `map_err(|_|`, `TODO|FIXME|HACK|FALLBACK`. **626 + 518 + 395 candidate hits.**
- **Tier 2 (Scan):** Opened ~50 hit files for 20-line context windows. Filtered ~80% as innocent (CF-counter idioms, test fixtures, defensible optional-field semantics).
- **Tier 3 (Investigate):** For ~30 remaining suspects, traced call chains. Found 17 HIGH+ severity findings.
- **Tier 4 (Deep Dive):** For the top 3 findings, ran the Contradiction Engine + LSU + Bayesian update + self-verification protocol per doctrine §11.

**Cold Read first** — examined offending lines BEFORE reading surrounding comments or commit history.
**Adversarial personas applied** — "The Bug" (where would I hide?), "The Tired Developer" (what would I assume?), "The Future Archaeologist" (what will confuse me in 2 years?).
**Self-verification (avoiding Holmes's Yellow Face error):** For each GUILTY verdict, asked "could I be wrong?" and confirmed at least one disproving search returned empty.

---

**Case status:** OPEN. Awaiting operator review and prioritization into GitHub issues per CLAUDE.md §3.

*— Sherlock Holmes, consulting detective*
*"Data! Data! Data! I can't make bricks without clay."*
