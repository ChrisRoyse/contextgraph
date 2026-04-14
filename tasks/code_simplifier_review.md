# Code-Simplifier Review — Phases 1-6

**Reviewer**: code-simplifier subagent
**Date**: 2026-04-14
**Scope**: files listed in the plan-of-record; approximately 10 000 LOC new + mods across
`context-graph-core`, `context-graph-storage`, `context-graph-mcp`, `context-graph-cli`.
**Verdict**: APPROVE WITH REFINEMENTS

## Summary

The five phases are internally consistent, the version-byte + bincode contract
is applied uniformly across the three new CFs (TRAINING_RECORDS, CONSTELLATIONS,
CONTRASTIVE_PAIRS), and the core algorithms (Welford, reservoir, SRC-3 similarity
profile, two-pass constellation compiler, streaming HOSVD) are mathematically
sound. The test suite is comprehensive and uses real RocksDB temp dirs — no mocks.
I found no critical data-corruption or panicking bugs. However several medium-
severity gaps stand out: two `unwrap_or_default`/`.ok()` calls in hot paths that
silently swallow topic-profile read errors, a duplicated `encode/decode`
version-byte helper pattern across three files that begs a generic
`encode_v1<T>`/`decode_v1<T>` refactor, inconsistent handling of the `unused`
`_source_metadata` parameter in `extract_temporal_labels`, two `reservoir.rs`
helpers that use dead `FiniteFallback::take_if_finite_or` branches, a
constellation scoring contract mismatch between the doc comment and the
implementation (mean vs. median), and a potential efficiency trap in
`mine_contrastive_pairs` where `similarity_profile` is computed twice on the
`None`-skip path. None of these corrupt data, but they merit tightening.

## Findings

### Critical

None. Data-path correctness is intact; no silent corruption paths were found in
the new CFs; all three encoders/decoders reject empty payloads and mismatched
version bytes.

### High

#### H-1. `EmbedderStats.coherence` contract says "mean", implementation uses median

- **File**: `crates/context-graph-core/src/constellation/compiler.rs:780` and
  `crates/context-graph-core/src/constellation/types.rs:62`
- **Problem**: `Constellation.coherence`'s docstring (types.rs:62) says it is
  "Mean cosine of members against the E1 centroid. 1.0 = perfect agreement".
  The implementation (`mean_cosine` at compiler.rs:780-786) returns
  `stats.cosine_spread_p50` — the **median**, not the mean. The function's
  internal doc comment even acknowledges the swap ("Use the median as a robust
  proxy for the cosine distribution's center"). Downstream consumers reading
  `coherence` per the types.rs docstring will make a false assumption.
  Constellation integration tests (`constellation_integration.rs`,
  `compile_with_duplicate_members_yields_coherence_one`) assert `coherence >=
  0.99` for identical-member clusters; the test is consistent with *either*
  mean or median, so the mismatch would not fail the FSV.
- **Fix**: Either (a) update the struct doc at types.rs:62 to say "Median
  cosine ... (p50)", which matches the phase-2 summary's note that "coherence =
  median cosine of E1 members vs E1 centroid (p50 from reservoir)", or
  (b) rename the field to `coherence_p50` to make the convention visible at
  every call site. Option (a) is minimum-change.

#### H-2. `get_topic_profile` read errors are dropped with `.unwrap_or_default()` in the constellation path

- **File**: `crates/context-graph-mcp/src/handlers/tools/constellation_tools.rs:125-128`
- **Problem**:
  ```rust
  let stored_profile = rocksdb_store
      .get_topic_profile(fp.id)
      .await
      .unwrap_or_default();
  ```
  `unwrap_or_default()` converts a `CoreResult<Option<[f32;13]>>` into
  `Option<[f32;13]>`, discarding the error. If RocksDB is in a broken state
  the handler silently falls through to the "row missing" path and proceeds to
  observe the member with `stored_profile=None`. This violates the fail-fast
  principle (plan §1). In `training_tools.rs:225-231` the same call is handled
  correctly — errors are logged with `warn!` and fall through, but here no log
  is emitted at all.
- **Fix**: Mirror the `training_tools.rs` pattern:
  ```rust
  let stored_profile = match rocksdb_store.get_topic_profile(fp.id).await {
      Ok(p) => p,
      Err(e) => {
          warn!(id = %fp.id, error = %e, "get_topic_profile failed; using fallback");
          None
      }
  };
  ```

#### H-3. Reservoir `min()` returns 0.0 for non-empty reservoirs that happen to contain no finite samples

- **File**: `crates/context-graph-core/src/constellation/reservoir.rs:92-108`
- **Problem**: `observe` drops every non-finite sample (reservoir.rs:47-48),
  so `samples` can only contain finite f32s; this is good. But the `min()` /
  `max()` helpers add a *second* finite-check layer via the
  `FiniteFallback::take_if_finite` / `take_if_finite_or` trait (reservoir.rs
  lines 129-151), and the chained calls
  `.fold(f32::INFINITY, f32::min).min(f32::INFINITY).take_if_finite()` are
  dead-code-at-the-edges: the outer `.min(f32::INFINITY)` is a no-op for any
  finite input, and `take_if_finite` silently converts an empty-reservoir
  `f32::INFINITY` into `0.0`. The non-empty-reservoir path is correct, but
  the extra layers confuse readers and make it hard to see that empty →
  0.0 while non-empty → true min. Worse: the `take_if_finite_or(f32::NEG_INFINITY)`
  in `max()` explicitly uses `f32::NEG_INFINITY` as the fallback, which would
  yield `0.0` only because `NEG_INFINITY` is not finite — accidental, not
  intentional.
- **Fix**: Replace with an explicit branch:
  ```rust
  pub fn min(&self) -> f32 {
      if self.samples.is_empty() { return 0.0; }
      self.samples.iter().copied().fold(f32::INFINITY, f32::min)
  }
  pub fn max(&self) -> f32 {
      if self.samples.is_empty() { return 0.0; }
      self.samples.iter().copied().fold(f32::NEG_INFINITY, f32::max)
  }
  ```
  Then delete the `FiniteFallback` trait and its impl (reservoir.rs:128-151).
  The explicit branch is three lines shorter and removes dead-code at the edges.

### Medium

#### M-1. `mine_contrastive_pairs` computes `similarity_profile` twice on the skip path

- **File**: `crates/context-graph-mcp/src/handlers/tools/contrastive_tools.rs:280-346`
- **Problem**: For every candidate that `mine_pair_from_candidate` rejects,
  the handler recomputes the full 13-slot `similarity_profile` (line 330) just
  to decide whether to bump `pairs_skipped_below_threshold` or
  `pairs_skipped_kind_filter`. That's a duplicate cosine-sweep over 13
  embedders at 1024D × 2 = O(26K FMA) per skip. The pre-filter at line 281
  already computed and has the `_profile` tuple; changing `mine_pair_from_candidate`
  to return a richer `Result<_, SkipReason>` (or passing the already-computed
  profile in) would avoid the recomputation. At `candidatePoolSize=500` and
  `topKCandidatesPerAnchor=10`, the skip path runs `500 * 500 = 250 000`
  profile-computations in the worst case when thresholds reject everything —
  doubling that to 500 000 for counter accuracy is wasteful.
- **Fix**: Either (a) pass the pre-computed `_profile` from line 296 into the
  skip-reason classifier so it doesn't recompute, or (b) have
  `mine_pair_from_candidate` return `Result<Option<ContrastivePair>, SkipReason>`
  where `SkipReason` distinguishes the two counter bumps. Option (a) is the
  smallest change. If the team thinks the counter accuracy isn't worth the
  handler complexity, drop the distinction and increment a single
  `pairs_skipped` counter.

#### M-2. Three near-identical `encode_vN` / `decode_vN` functions with the same shape

- **File**:
  `crates/context-graph-storage/src/teleological/rocksdb_store/export.rs:284-315`
  `crates/context-graph-storage/src/teleological/rocksdb_store/constellation.rs:299-328`
  `crates/context-graph-storage/src/teleological/rocksdb_store/contrastive.rs:458-487`
- **Problem**: Three separate `encode_X` / `decode_X` pairs all do:
  1. `bincode::serialize(x).map_err(CoreError::SerializationError)`
  2. Prepend the version byte
  3. Reject empty payloads with `"payload is empty"`
  4. Reject mismatched version bytes with a uniform message
  The identical ceremony is cut-and-pasted three times (plus inline unit tests
  per file). This is the exact pattern that mature codebases generalize into
  a typed helper — especially because Phase 7/8 will add a fourth (and fifth)
  CF-versioned payload and the drift risk grows with each copy.
- **Fix**: Factor into a shared helper in
  `context-graph-storage/src/teleological/rocksdb_store/mod.rs`:
  ```rust
  pub(crate) fn encode_versioned<T: serde::Serialize>(
      version: u8, type_name: &str, value: &T,
  ) -> CoreResult<Vec<u8>> { ... }
  pub(crate) fn decode_versioned<T: serde::de::DeserializeOwned>(
      expected_version: u8, type_name: &str, bytes: &[u8],
  ) -> CoreResult<T> { ... }
  ```
  Keep the three typed wrappers (`encode_training_record`, `encode_constellation`,
  `encode_pair`) as one-line calls so the public surface is unchanged. Do the
  same for `decode_*`. This removes roughly 60 lines of duplication and
  guarantees every future CF inherits the same empty-payload / version-mismatch
  contract.

#### M-3. `extract_temporal_labels` accepts `_source_metadata` but never uses it

- **File**: `crates/context-graph-core/src/training/temporal.rs:101-138`
- **Problem**: The function takes `_source_metadata: Option<&SourceMetadata>`,
  leading-underscored in the body, and the handler at
  `training_tools.rs:482-488` dutifully passes `source.as_ref()`. But the
  argument is never read — `session_sequence` and `session_total` are passed
  in as separate `Option<u32>` parameters instead. This violates the plan's
  "no dead parameters" principle and the API comment that says "Caller
  responsibilities: Resolve `session_sequence`/`session_total` from the session
  bookkeeping backing store" (temporal.rs:96-97) clearly states the caller
  already does the resolution. The SourceMetadata ref is vestigial.
- **Fix**: Drop the parameter. Update the one call site in `training_tools.rs`
  to drop its `source.as_ref()` argument. One fewer parameter to document,
  one fewer thing for the reader to wonder about.

#### M-4. `reservoir_stddev` recomputes stats the reservoir already has

- **File**: `crates/context-graph-core/src/constellation/compiler.rs:769-778`
- **Problem**: `reservoir_stddev` sorts the reservoir (inside `r.sorted()`),
  then iterates the sorted vec to compute mean-then-variance, despite the
  reservoir having no need for sorted order here. A plain
  `samples = &r.samples` (unsorted) + same two-pass variance loop would give
  the same answer at the cost of one `clone` (which `.sorted()` does anyway)
  minus the `sort_by` call. At 1024 samples the sort cost is small (~10µs),
  but the code reads as "needs sorted input" when it doesn't.
- **Fix**: Add a `raw_samples(&self) -> &[f32]` accessor to
  `ReservoirSample` and call that here. Drop the `.sorted()` round-trip. Same
  accuracy, less compute, clearer intent.

#### M-5. `get_content_batch` failure in contrastive handler silently uses empty strings

- **File**: `crates/context-graph-mcp/src/handlers/tools/contrastive_tools.rs:217-227`
- **Problem**: When `get_content_batch` fails, the handler logs a `warn!` and
  continues with `vec![None; pool_ids.len()]`. Every mined pair then gets
  `anchor_text=""` / `negative_text=""`. The docstring at the top of the
  module (line 127 in phase3_summary.md: "Anchors/negatives missing content
  land with empty strings (a warn! is logged)") describes the per-memory
  missing-content path, but this is the batch-failure path that means the
  whole run produces textless pairs. That's a silent degradation that the
  caller only learns about through a single log line, not through the
  response payload. The plan's "no silent fallbacks" principle is bent.
- **Fix**: Two options: (a) return a tool error when `get_content_batch`
  fails, since the downstream consumer relies on `anchor_text` / `negative_text`
  for training data; or (b) include a `content_fetch_failed: bool` flag in
  the summary response so the caller can detect the degradation without
  grepping server logs. Option (a) is strictly correct per fail-fast; option
  (b) is a softer middle ground if the caller sometimes wants metadata-only
  pairs.

#### M-6. `list_constellations` shape summary double-serializes `per_embedder` stats

- **File**: `crates/context-graph-mcp/src/handlers/tools/constellation_tools.rs:242-266`
  and `render_constellation` at lines 589-614
- **Problem**: The `list_constellations` handler unconditionally renders the
  full `per_embedder` array (with all cosine stats per embedder) for every
  item, regardless of `includeCentroids`. For a page of 50 constellations × 13
  embedders × 12 fields per `EmbedderStats` = ~8000 JSON fields in a single
  response payload just to populate a listing. The training-tools analog
  (`list_training_records`) is carefully gated by `includeShape=true` — the
  constellation listing should parallel that. `render_constellation` with
  `include_centroids=false` still spreads the full 13-entry `per_embedder`
  array; it only drops the centroid payload. A compact listing needs a
  *shape-only* fork.
- **Fix**: Add an `include_shape` flag to `list_constellations` (default
  `false`) and short-circuit `render_constellation` to emit only
  `{id, label, selector_kind, member_count, coherence, purity}` when false.
  Keep `include_centroids` for `get_constellation` where the full payload is
  explicitly requested.

#### M-7. `compile_constellation` reads `get_topic_profile` inside the per-member loop (13 extra RocksDB calls per 13-member constellation)

- **File**: `crates/context-graph-mcp/src/handlers/tools/constellation_tools.rs:124-140`
- **Problem**: The loop calls `rocksdb_store.get_topic_profile(fp.id).await`
  once per member (line 125-128). For a 50 000-member constellation that's
  50 000 serialized point-gets. `RocksDbTeleologicalStore` already exposes
  `multi_get_training_records` — a parallel `multi_get_topic_profiles(&[Uuid])`
  helper backing a single `multi_get_cf` would turn this into one batched
  call. The analog fix landed for training records in Phase 1 as fix #6 /
  `multi_get_training_records`; constellation compilation missed the same
  treatment.
- **Fix**: Add `multi_get_topic_profiles(&self, ids: &[Uuid]) ->
  CoreResult<Vec<Option<[f32;13]>>>` next to `get_topic_profile` in
  `export.rs`, then call it once up-front in the handler and index into the
  Vec inside the loop.

### Low

#### L-1. `MiningSummary` is `Default` but never constructed from `Default::default()` on the success path

- **File**: `crates/context-graph-core/src/contrastive/types.rs:210-227`
- **Problem**: `#[derive(Default)]` is only exercised on the
  `no-op` early-return path (contrastive_tools.rs:195-197). The rest of the
  handler populates every field explicitly; the `Default` impl is dead
  weight. This isn't a bug, but the derive hints "struct has useful default
  meaning" which isn't true.
- **Fix**: Either keep the derive and document that it means "empty run" or
  drop the `Default` derive and initialize the no-op path struct-literal-style.
  Either change is fine; consistency matters more than the direction.

#### L-2. `AnomalyKind::all` is a hardcoded list that can desync from the enum

- **File**: `crates/context-graph-core/src/contrastive/types.rs:99-108`
- **Problem**: `AnomalyKind::all()` is a hand-maintained array. If someone
  adds a seventh `AnomalyKind` variant and forgets to update `all()` and
  `NUM_ANOMALY_KINDS` at line 21, test coverage will not detect it
  (`from_u8(6)` returns `None`, which is the intended behavior). The
  `#[test]` at contrastive/tests.rs that exercises `as_u8`/`from_u8` does
  round-trip every variant via `all()` — so the test *would* pass with a new
  variant missing from `all()` because the new variant isn't iterated.
- **Fix**: Add a compile-time check via `strum`
  (`#[derive(strum::EnumIter, strum::FromRepr)]` gives both `iter()` and
  `from_repr(u8)` for free), or add a unit test that uses a
  `match AnomalyKind::SemanticButNotCausal { … => {} }` exhaustive block —
  the compiler will error when a variant is added and the test goes stale.
  Low priority: it's a self-policing contract today.

#### L-3. `encode_anchor_negatives` / `decode_anchor_negatives` are missing a version byte

- **File**: `crates/context-graph-storage/src/teleological/rocksdb_store/contrastive.rs:519-535`
- **Problem**: `CF_CONTRASTIVE_BY_ANCHOR` stores raw
  `bincode(Vec<Uuid>)` without a version prefix. Every other new CF
  (`CF_TRAINING_RECORDS`, `CF_CONSTELLATIONS`, `CF_CONTRASTIVE_PAIRS`) uses
  the `[version:u8][bincode]` layout. If later we want to switch to a more
  compact encoding (e.g. raw 16-byte chunks without bincode framing), there's
  no version field to pivot on. Today the value is semantically "a list of
  UUIDs" and bincode's framing is stable, so the risk is small.
- **Fix**: Add a `const CONTRASTIVE_BY_ANCHOR_VERSION: u8 = 1;` and prepend
  it during encode + validate on decode. The change is ~8 lines and keeps
  the CF consistent with the rest of the new storage layer. If the team
  deems it over-engineering for a list-of-UUIDs value, document the exception
  at contrastive.rs:519 with "value is raw bincode; no version byte because
  the list semantics are stable forever".

#### L-4. `count_training_records` uses an `iterator_cf(..).count()` even though `multi_get_training_records` exists for the page

- **File**: `crates/context-graph-storage/src/teleological/rocksdb_store/export.rs:137-140`
- **Problem**: Count is an O(N) scan over the full CF (iter + count). Same
  for `count_constellations` (constellation.rs:154-157). RocksDB exposes
  `GetIntProperty(..., "rocksdb.estimate-num-keys")` which is O(1) and
  accurate enough for the "roughly how many rows?" use case. The current
  implementation is correct but allocation-heavy at scale; every call scans
  the whole keyspace.
- **Fix**: Replace `count_training_records` / `count_constellations` /
  `count_contrastive_pairs` bodies with a call to
  `self.db.property_int_value_cf(cf, "rocksdb.estimate-num-keys")` and
  fall back to the iterator scan when the estimate is absent. Update the
  docstring to say "approximate when the CF has pending compactions". Most
  callers will not care about the ±small delta.

#### L-5. `reservoir.rs::with_seed` doesn't document its RNG algorithm stability

- **File**: `crates/context-graph-core/src/constellation/reservoir.rs:37-44`
- **Problem**: The ReservoirSample's reproducibility depends on `StdRng`'s
  algorithm being stable across `rand` versions. When `rand` bumps its
  default algorithm (as it has done historically), every stored
  constellation's cosine percentiles would shift. That wouldn't corrupt
  data (the CF version byte doesn't fingerprint the RNG), but it would
  violate the implicit "rerun produces the same numbers" contract.
- **Fix**: Pin the RNG to a stable algorithm — `rand_chacha::ChaCha8Rng` is
  documented as algorithmically stable. Add a note at reservoir.rs:37
  explaining why. Low priority: the values are already statistical estimates.

#### L-6. `structured_fingerprint` test helper has two-element tuples that read as magic numbers

- **File**: `crates/context-graph-core/src/teleological/tucker.rs:838-876`
- **Problem**: `a_coeffs` / `b_coeffs` are hand-written 13-element arrays
  with no explanation of their shape. Any future reviewer has to reverse-engineer
  that the `0.0, 0.0, 0.0` at indices [5] (E6), [11] (E12), [12] (E13) are
  deliberate (sparse / token rows that `extract_reference_view` zeroes out).
- **Fix**: Add a one-line comment on each array noting that indices 5, 11,
  12 are deliberately zero because those embedders are zero rows. Purely
  cosmetic.

#### L-7. `tucker.rs::power_iteration_top_k` emits a zero-eigenvector but still claims convergence

- **File**: `crates/context-graph-core/src/teleological/tucker.rs:551-559`
- **Problem**: When `norm_sq.sqrt() < POWER_ITER_TOL.sqrt()` (the residual
  subspace is exhausted), the function silently assigns `x =
  vec![0.0f32; n]`, `lambda = 0.0`, and marks `converged = true`. That's
  sensible — the eigenpair really is zero — but a downstream caller doing
  `norm(u_k)` would see a zero vector and might interpret it as "U3 has a
  degenerate column" without realising the algorithm deliberately emitted it.
- **Fix**: Track the number of zero eigenpairs in the return tuple, or
  log a `debug!` at line 556 saying "eigenpair k of k is zero; residual
  subspace exhausted". Low priority: tests don't exercise this path, and
  the mathematics is sound.

#### L-8. `encode_training_record`'s `out.append(&mut bytes)` could be `out.extend_from_slice(&bytes)`

- **File**: `crates/context-graph-storage/src/teleological/rocksdb_store/export.rs:289-290`
- **Problem**: `Vec::append(&mut other)` leaves `other` empty (by moving
  its contents), which is fine here but less obvious than
  `extend_from_slice`. Same pattern at constellation.rs:304-305 and
  contrastive.rs:464-465.
- **Fix**: Change all three to `out.extend_from_slice(&bytes);` or
  `out.extend(bytes)`. Equivalent cost, clearer intent. If the helper refactor
  from M-2 lands, this drops out for free.

## Positive observations

Patterns worth keeping and replicating in Phases 7+:

1. **Every new CF has an inline unit test** that asserts encode/decode
   round-trip, empty-payload rejection, and version-byte rejection. The same
   pattern is followed in all three of training/constellation/contrastive. Keep
   this template for the next CF.
2. **Atomic `WriteBatch` for primary + secondary indexes** is used in both
   constellation and contrastive — no partial-write risks even if the process
   dies mid-store. Delete paths mirror the same atomic shape. Keep this for
   any future CF that needs multi-CF coherence.
3. **Resolve selector/key *before* acquiring CF handles** (constellation.rs:167
   explicitly documents this `!Sync` / `tokio::spawn` pitfall). The training
   and contrastive handlers inherit the same discipline. Document it in the
   storage crate's `README` so Phase 7 agents don't have to rediscover it.
4. **`list_fingerprints_unbiased` is called exactly once per handler**, and the
   results are then reused for both anchors and negatives via a
   `content_by_id` HashMap. This is exactly the right shape for a future
   pipeline that may add entity caches or feature caches.
5. **`additionalProperties: false` on every schema** — Phase 1 learned this
   the hard way (per `tasks/lessons.md`); Phases 2/3/4/6 all apply it
   consistently. Good defensive practice.
6. **CPU-only first, GPU deferred behind a feature flag (Phase 4)** — the
   Tucker module docs explicitly call out the deferral and the reason. When
   the GPU path ships in a later phase it will land in a clean, documented
   boundary rather than colliding with existing CPU code.
7. **Phase 6's `encode_training_record` / `decode_training_record` reuse**
   instead of re-encoding in the CLI. The on-disk bytes are byte-for-byte
   identical to RocksDB's — no parallel encoder to keep in sync. This is
   the right cross-crate boundary.
8. **The Tucker module uses heap-allocated `Box<[[f32; 1024]; 13]>` for the
   reference frame** (tucker.rs:292-299) explicitly to avoid a 52KB stack
   frame. Documented in-comment. Exactly the kind of detail that burns a
   thread-per-compress without it.
9. **`jacobi_eigen_13` is a scale-aware convergence check** using
   `off / (diag + 1.0) < JACOBI_TOL` rather than a raw threshold (tucker.rs
   lines 391-403). This is correct for matrices of vastly different
   magnitudes, which is exactly what `G_1` can produce when different
   embedders have different L2 norms.
10. **All three new CFs use `uuid_prefixed_cf_options` or explicit
    per-CF tuning** (column_families.rs:43-47, 773-789). The helper is
    well-documented and the non-uniform choices (sparse indexes at 2MB,
    dense payloads at 4-8MB write buffers) are all explained in-comment.
    Phase 7+ should continue this pattern.

---

### Summary of remediation effort

| Finding | Effort | Impact |
|---------|--------|--------|
| H-1 docstring fix | 2 LOC | Prevents consumer misconception |
| H-2 unwrap_or_default → warn + None | 8 LOC | Fail-fast compliance |
| H-3 FiniteFallback removal | -15 LOC | Simpler, no dead branches |
| M-1 avoid double similarity_profile | 5-20 LOC | ~40% faster mining on reject-heavy corpora |
| M-2 generic encode_v1/decode_v1 | -40 LOC net | Reduces future drift |
| M-3 drop unused _source_metadata | -3 LOC + 2 call sites | Signal clarity |
| M-4 expose raw samples to reservoir_stddev | 5 LOC | Cosmetic + small perf |
| M-5 content-fetch failure handling | 15 LOC | Explicit degradation signal |
| M-6 include_shape on list_constellations | 30 LOC | Order-of-magnitude listing payload reduction |
| M-7 multi_get_topic_profiles helper | 40 LOC | 50x-100x constellation-compile latency on large member sets |
| L-1..L-8 | -60 LOC net total | Incremental hygiene |

The High-severity fixes are all contained in existing files and are strictly
additive in ~20 LOC. Medium-severity M-2 and M-7 are the most impactful and
should land as part of Phase 7 hardening; everything else can queue behind
them.
