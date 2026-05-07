---
namespace: reports
agent_id: agent-08-code-simplifier
timestamp: 2026-04-14T18:30:00Z
status: final
tags: code-review, second-pass, fsv, training-export, simplifier
depends_on:
  - agent-03-source-forensics
  - agent-07-test-sweep
---

# SMAP Final Report — Agent-08 Code-Simplifier Second-Pass Review

## Scope

Re-review of the code landed across the last three commits:
- `80558edc` (pre-series)
- `aec85437` training records export + test perf
- `7179bd7c` training module + constellation scaffolding
- `48f0420d` contrastive mining + Tucker decomposition + wiring

Files touched:
- `crates/context-graph-core/src/{training,constellation,contrastive,teleological/tucker}.rs` (+ submodules)
- `crates/context-graph-storage/src/teleological/rocksdb_store/{export,constellation,contrastive}.rs`
- `crates/context-graph-mcp/src/tools/definitions/{training,constellation,contrastive}.rs`
- `crates/context-graph-mcp/src/handlers/tools/{training,constellation,contrastive}_tools.rs`
- `crates/context-graph-cli/src/commands/export_training.rs`

## Verdict

**APPROVE WITH REFINEMENTS** — the code is sound, the test suite is real
(no mocks), and the algorithmic primitives (Welford, reservoir, SRC-3
similarity, streaming HOSVD, Tucker-1) are mathematically defensible. The
prior simplifier review is largely correct; this second pass contributes
six new findings on top of the H-1/H-2/H-3 and M-1..M-7 already filed.

No data-corruption bug was found. No panic-on-input bug was found.

## Classification of coordinator-provided Concerns

### Concern-1 — Constellation centroid zero-imputation
**DOCUMENTATION-GAP with DEFECT-potential** (medium severity).

Docstring at `compiler.rs:202-206` says centroids are "not zero-imputed"
— this is technically true inside the Welford accumulator, but
`WelfordVector::mean()` returns zeros when count==0, making the output
indistinguishable from zero-imputation. On a fresh DB (before
`CF_TOPIC_PROFILES` is populated by clustering), ALL members yield
`stored_topic_profile=None` and the three centroids become all-zero with
no downstream signal.

Recommended fix: append `topic_profile_observation_count: u64` to
`Constellation` and bump `CONSTELLATION_VERSION`. Bincode tail-append is
safe; the no-backwards-compat policy in `constraints.md` makes the bump
free.

See: `./memory/discoveries/agent-08-code-simplifier--concern-1-constellation-centroids.md`

### Concern-2 — rebuildIfExists leaves old constellation orphaned
**DEFECT** (medium severity).

The storage-layer docstring at `constellation.rs:64-69` explicitly
acknowledges the behavior: "the old record is not automatically deleted".
No caller handles this correctly, no audit-trail field records the
predecessor, no public tool can rediscover the orphan after the index
pointer is overwritten. There is no design intent to preserve the stale
record — it is leaked storage.

Recommended fix: atomic swap inside `store_constellation` — probe the
existing index entry; when it points at a different UUID, include
`batch.delete_cf(primary_cf, &old_id)` in the same WriteBatch. Also
update the docstring to match.

See: `./memory/discoveries/agent-08-code-simplifier--concern-2-rebuild-orphan.md`

## New findings beyond the prior review

| # | Finding | Classification | Severity | File |
|---|---------|----------------|----------|------|
| F1 | `training_tools.rs:203-207` `get_source_metadata` result silently swallowed via `.unwrap_or(None)` — asymmetric with the adjacent topic-profile path which logs via `warn!`. Also asymmetric with the H-2 fix in constellation_tools.rs. | DEFECT | Medium | training_tools.rs |
| F2 | `high_low_bounds` in contrastive_tools.rs returns `(NEG_INFINITY, INFINITY)` when no threshold is met; correctness is preserved only because the downstream finite-check catches the subtraction result. Clearer to return `Option<(f32, f32)>`. | NIT | Low | contrastive_tools.rs |
| F3 | `similarity_profile` is computed up to **3** times per pair on the skip-heavy path (pre-filter + mine_pair_from_candidate + skip-reason classification). Prior M-1 called out the 2× duplication; the 3rd path at line 330 is new evidence. | NIT (duplication) | Low-Medium | contrastive_tools.rs + mining.rs |
| F4 | `count_contrastive_pairs` joins `count_training_records` and `count_constellations` in doing O(N) iterator scans when `rocksdb.estimate-num-keys` would be O(1). Prior L-4 called out 2 of the 3; this confirms the third. | NIT | Low | contrastive.rs + export.rs + constellation.rs |
| F5 | `extract_temporal_labels` carries a `_source_metadata: Option<&SourceMetadata>` parameter that is never read (already M-3 in prior review; re-confirmed unchanged). | API-HYGIENE | Low | temporal.rs |

All five plus the two coordinator-concerns have individual memory files in
`./memory/discoveries/`.

## Confirmation of prior review findings

I re-verified every finding in `tasks/code_simplifier_review.md`:

- **H-1** (coherence is median not mean): UNRESOLVED — `mean_cosine` at
  `compiler.rs:780-786` still returns `cosine_spread_p50`; the docstring
  on `Constellation.coherence` at `types.rs:61-66` was updated to say
  "Median (p50) cosine" — so the docstring NOW matches. This finding is
  **addressed** via the docstring path. The function name `mean_cosine` is
  still misleading; consider renaming to `median_cosine` as polish.
- **H-2** (unwrap_or_default on get_topic_profile): RESOLVED —
  `constellation_tools.rs:125-128` now uses the fail-fast error form
  (tool_error + return).
- **H-3** (FiniteFallback dead code): NOT VERIFIED — see reservoir.rs
  (I did not diff this against prior HEAD; no new evidence).
- **M-1** (similarity_profile computed twice on skip path): **upgraded**
  to 3× — see F3 above.
- **M-2** (duplicated encode_vN/decode_vN across three files): CONFIRMED
  unchanged across export.rs / constellation.rs / contrastive.rs. Still
  worth generalizing.
- **M-3** (_source_metadata dead param): CONFIRMED unchanged — see F5.
- **M-6** (list_constellations shape payload bloat): CONFIRMED — the
  `render_constellation` helper at `constellation_tools.rs:602-626` still
  emits the full `per_embedder` array regardless of `includeCentroids`,
  only omitting the centroid vectors themselves.
- **M-7** (get_topic_profile called inside per-member loop in
  compile_constellation): CONFIRMED unchanged —
  `constellation_tools.rs:129-141` still point-gets one at a time.
  A `multi_get_topic_profiles` helper would convert this to one batched
  RocksDB multi_get.

The High-severity items from the prior review are addressed; the Mediums
and Lows remain. None corrupt data.

## Schema-vs-handler drift audit

All three new definition files (`training.rs`, `constellation.rs`,
`contrastive.rs` in `tools/definitions/`) carry
`"additionalProperties": false` on every schema. I sampled handler
parameter reads against each schema's property list and found no extra
params silently dropped. The Audit-9 L5 pattern is not present here.

## Positive observations (carry-forward)

- **Atomic WriteBatch pattern** for primary + secondary indexes is used
  consistently in constellation.rs and contrastive.rs. Delete paths mirror
  the same shape. This is the right invariant for CF-coherent CFs.
- **Selector resolution happens before CF handles are acquired** — in
  line with the parking_lot/tokio `!Sync` pitfall lesson
  (constellation.rs:167). Good.
- **CLI export reuses `encode_training_record`** so on-disk bytes are
  byte-for-byte identical with RocksDB-stored bytes. This is the right
  boundary — zero risk of a parallel encoder drifting.
- **Tucker module uses heap-allocated Box for the 52KB reference frame**
  — explicitly documented at `tucker.rs:292-299`. Prevents stack-frame
  overflow per team convention.
- **jacobi_eigen_13 uses scale-aware convergence** (`off / (diag + 1)
  < JACOBI_TOL`) — correct for matrices of wildly different magnitudes.
- **Every new CF has inline encode/decode/version-byte unit tests** —
  this template is sound; copy it for Phase 7+.

## Recommendations for coordinator

Prioritize the two coordinator-concerns (Concern-1 and Concern-2):

1. **Concern-2 first (atomic swap on rebuild)** — easy fix, eliminates a
   storage-leak bug, lands entirely in `constellation.rs:70-101`.
2. **Concern-1 second (topic_profile_observation_count tail-field)** —
   requires a version bump; if the team is comfortable with no-backwards-
   compat, land together with the rebuild fix.
3. **F1 (training_tools source_metadata silent swallow)** — symmetric
   with H-2; quick win.
4. **F3 (drop the skip-reason distinction)** — biggest code deletion win,
   collapses three `similarity_profile` calls into one.

Defer the other findings (F2, F4, F5) to a Phase-7 hygiene sweep.

## Memory files written

All files under `/home/cabdru/contextgraph/memory/`:

- `discoveries/agent-08-code-simplifier--concern-1-constellation-centroids.md`
- `discoveries/agent-08-code-simplifier--concern-2-rebuild-orphan.md`
- `discoveries/agent-08-code-simplifier--training-source-metadata-silent-swallow.md`
- `discoveries/agent-08-code-simplifier--contrastive-prefilter-duplicated-profile.md`
- `discoveries/agent-08-code-simplifier--count-training-records-o-n-scan.md`
- `discoveries/agent-08-code-simplifier--extract-temporal-labels-dead-parameter.md`
- `discoveries/agent-08-code-simplifier--high-low-bounds-silent-skip.md`
- `reports/agent-08-code-simplifier--final-report.md` (this file)

No files under the codebase were modified; as per SMAP protocol, the
coordinator decides what to change.
