# Agent Handoff Prompt — `#308` Rust Refactor for Parallel-Isolated MPS Speedup

Copy-paste everything below this line into a fresh agent session. The agent should follow the project doctrine (`/home/cabdru/contextgraph/docs2/AICodingAgentSuperPrompt.md`), the aiwonder operations rules (`/home/cabdru/contextgraph/docs2/aiwonder.md`), and the FSV protocol throughout.

---

## Goal (one sentence)

Refactor the Rust runner `python_forward_cache_fsv` and the Python scheduler `consolidate_isolated_subruns` so that parallel-isolated mode (`CG_MEJEPA_PY_FORWARD_PARALLEL_EMBEDDERS > 1`) on aiwonder produces a complete, byte-correct `packed_records/` for all 12 embedders — unlocking the additional ~2.4× MPS-overlap speedup on top of the current 2.27× aiwonder sequential speedup over WSL.

## Why this matters

The aiwonder production run (issue `#296` EPIC, tmux `cg_pydata016_aiwonder_prod_20260515-191116Z`) is currently running at ~587 s/shard sequential = **~2.64 days** to finish 391 shards. With this refactor, parallel mode brings each shard to ~230 s = **~1 day total**. That's ~1.5 days of compute saved + future runs benefit forever.

Two prior commits already shipped the prerequisite race-fix locks (`65d99b0c`, `b44926a3`); the threading.Lock around `promote_hot_stage_shard` unblocked the byte-count race in `promote_tree_verified`, but a **separate** invariant fails at consolidation: `shard.shard_id isolated packed rows match index rows`. This is the bug to fix.

## Context — what's already true

1. **Repo state.** HEAD is on `main` (push commit `65d99b0c` or later). The relevant files:
   - `crates/context-graph-mejepa-embedders/examples/python_forward_cache_fsv.rs` (4785 lines — the Rust runner)
   - `scripts/swebench_lite_ops/python_forward_shard_scheduler.py` (~5100 lines — the Python scheduler; locks already in place)

2. **The bug.** Each Rust subrun, when invoked via `CG_MEJEPA_PY_FORWARD_ONLY_EMBEDDER=<slug>`, reads existing durable `cache_root/packed_records/` at startup and **forward-copies** the existing files into its own stage `packed_records/` before writing its own embedder's outputs. In sequential mode this works — each subrun sees the durable cache populated with its predecessors' data and copies it forward. In parallel mode, multiple subruns start simultaneously, each reads an empty (or stale) durable, each stages only its own embedder's data, then races on promotion. After `_PROMOTE_HOT_STAGE_LOCK` serializes the moves, only the last winner's data survives at durable because `promote_tree_verified` archives the prior destination and replaces it.

3. **Forensic evidence.**
   - Successful sequential smoke: `/zfs/archive/contextgraph/fsv/python-forward-cache-v1-aiwonder-smoke-20260515-185853Z-fsv/run_logs/shard-000000-offset-000000000.json` — shard wall 562 s, 12 subruns × `cache_record_count` summing to 11 880, final `packed_records/` has all 12 embedder pairs.
   - Failed parallel=4 smoke: `/zfs/archive/contextgraph/fsv/python-forward-cache-v1-aiwonder-302-fix-smoke-20260515-191626Z-fsv/` — all 12 subruns exit 0 in 231 s wall but consolidation fires `shard-000000-offset-000000000 isolated packed rows match index rows` at scheduler `python_forward_shard_scheduler.py:1780`.

4. **Live state.**
   - Aiwonder production run is in tmux `cg_pydata016_aiwonder_prod_20260515-191116Z` — **do not stop it.** It will finish in ~2.5 days on the current (sequential) path. The refactor work below happens in a SEPARATE smoke + restart at the end if the work is verified clean.
   - Local WSL fix-test in tmux `cg_pydata016_fixtest_181610` on the operator's WSL — keep it alive too; it's calibration data.

## Atomic scope

Land **one architecturally clean change** that:

1. **Rust runner:** stop reading existing durable `packed_records/` at startup. Each subrun writes ONLY its own embedder's `<slug>.jsonl` and `<slug>.index.jsonl` into a per-subrun output directory (do NOT touch other embedders' files). The runner's `packed_manifest.json` describes only the subrun's own contribution.

2. **Python scheduler `consolidate_isolated_subruns`:** after all subruns complete and snapshots are captured, **merge** the per-subrun packed_records into the durable top-level `packed_records/`. Specifically:
   - Walk each snapshot's `packed_records/<slug>.jsonl` + `<slug>.index.jsonl`
   - Copy (or hard-link) them into `shard.cache_root/packed_records/<slug>.jsonl`
   - Reconstruct the `python_forward_cache_packed_manifest.json` with `row_count = sum(per-embedder rows)` and `per_embedder = { slug: { record_path, record_sha256, byte_count, index_path, index_sha256, row_count } for each slug }`
   - Re-assert all the same invariants at lines 1768-1791 in the existing consolidation code

3. **Keep sequential mode bit-equivalent.** The default `CG_MEJEPA_PY_FORWARD_PARALLEL_EMBEDDERS=1` path must produce **byte-equal** durable state to the pre-refactor sequential path on the same input. Snapshots in `isolated_subrun_rows/` must be bit-identical. This is what proves the refactor is non-destructive.

4. **No breaking changes to the spec.** `--active-embedder-count` stays at 12. The packed_manifest schema may add fields but must not remove any. The `count_packed_index_rows` helper at line 3828 may need an update if the layout changes.

## Source of Truth

For every change:

- **Code:**
  - `crates/context-graph-mejepa-embedders/examples/python_forward_cache_fsv.rs` (Rust runner, must rebuild)
  - `scripts/swebench_lite_ops/python_forward_shard_scheduler.py` (Python scheduler, lines 1652–1791 + 3828)
- **Existing tests:**
  - `cargo test -p context-graph-mejepa-embedders` (whatever's in that crate)
  - `python_forward_cache_fsv.rs` example must build and run via `cargo build --release -p context-graph-mejepa-embedders --example python_forward_cache_fsv` on aiwonder.
- **FSV artifacts to produce:**
  - Sequential-mode regression smoke at `/zfs/archive/contextgraph/fsv/aiwonder-308-fix-seq-regression-<TS>-fsv/`
  - Parallel=4 fix smoke at `/zfs/archive/contextgraph/fsv/aiwonder-308-fix-par4-<TS>-fsv/`
  - Parallel=6 fix smoke at `/zfs/archive/contextgraph/fsv/aiwonder-308-fix-par6-<TS>-fsv/`
  - All must have `shard-000000-offset-000000000.json` final log with `exit_code=0`, `cache_record_count` matching expected (sum of per-embedder for the corpus rows touched), and `passes=true`.

## FSV acceptance criteria

**Happy path A — Sequential regression:**
- Run 1-shard smoke with `--shard-count 1`, no parallel env var. Output `packed_records/` byte-equal to the existing successful sequential smoke artifact at `/zfs/archive/contextgraph/fsv/python-forward-cache-v1-aiwonder-smoke-20260515-185853Z-fsv/` (compare sha256 of every per-embedder `<slug>.jsonl` and `<slug>.index.jsonl`).
- Same `cache_record_count = 11880`, same `chunk_count = 1024`.

**Happy path B — Parallel=4 success:**
- Run 1-shard smoke with `CG_MEJEPA_PY_FORWARD_PARALLEL_EMBEDDERS=4`.
- All 12 subruns exit 0.
- Final shard log present with `cache_record_count = 11880` (same as sequential), `passes=true`.
- Wall time under 400 s (vs sequential 562 s = ≥30% speedup).

**Happy path C — Parallel=6 success:**
- Same as B with `CG_MEJEPA_PY_FORWARD_PARALLEL_EMBEDDERS=6`.
- Wall under 300 s (≥45% speedup vs sequential).

**Edge case 1 — A single embedder is quarantined.** Synthetic input where e7's runner is forced to quarantine all chunks. Consolidation handles `per_embedder[e7].row_count = 0` cleanly.

**Edge case 2 — Empty durable cache_root before run.** Fresh cache_root (no prior promotions). All 12 subruns write to their stages, consolidation merges to top-level packed_records. (This was the failing case for the prior parallel mode.)

**Edge case 3 — Resume from completed shard.** Shard 0 done sequentially; run shard 1 in parallel-4 mode. Resume mechanism (`--resume-completed-shards`) skips shard 0 and produces correct shard 1 packed_records.

**Edge case 4 — Concurrent subruns + concurrent shard-level promotion.** With parallel-4 mode, all 12 subruns finish in ~120 s; the lock-serialized promotions queue up; final merge step runs and produces correct manifest.

## Detailed procedure

1. **Read the existing code (LSU — don't skim summaries):**
   - `crates/context-graph-mejepa-embedders/examples/python_forward_cache_fsv.rs:135-3045` — focus on `packed_record_path` (line 789), the forward-copy logic (search for `read_existing_packed` or wherever durable cache is enumerated at startup), and the `EmbedderCoverageStats` aggregation (line 281).
   - `scripts/swebench_lite_ops/python_forward_shard_scheduler.py:1652-1791` — the consolidation; `1768-1791` is where the packed_manifest invariants fire.
   - `scripts/swebench_lite_ops/python_forward_shard_scheduler.py:3828-3850` — `count_packed_index_rows`.

2. **Design — pick one of two paths:**
   - **(a) Strict per-subrun isolation:** Rust runner outputs to `cache_root/packed_records/<slug>.jsonl` and `<slug>.index.jsonl` (current layout), but **never reads** existing durable state. Each promote_hot_stage_shard puts only that subrun's files into durable. **Python consolidation** then collects all 12 per-subrun files from the snapshot dirs, copies them to the final durable `packed_records/`, and writes the merged manifest. This is the cleanest path.
   - **(b) Sub-directory layout:** Rust runner outputs to `cache_root/packed_records/by_embedder/<slug>/{record.jsonl,index.jsonl}`. Sequential mode is unchanged (Rust still reads existing if it wants). Consolidation walks `by_embedder/<slug>/` and merges into the top-level. More layout churn, more reader updates required.
   - **Recommendation: (a)**, because the consolidation step already exists and just needs the merge step added.

3. **Implement (a):**
   - In Rust runner, find the "read existing durable packed_records" code and **disable it under a flag** like `CG_MEJEPA_PY_FORWARD_SKIP_EXISTING_PACKED_READ=1`. Keep the default behavior bit-equivalent. Document the flag.
   - In Python `consolidate_isolated_subruns` (line 1652), after the per-snapshot index/quarantine/batch_telemetry merge (line 1763), add a **packed_records merge step**:
     1. Initialize empty `shard.cache_root/packed_records/` (or overwrite existing).
     2. For each snapshot (per slug), find the snapshot's source packed dir (via `snapshot["files"]["manifest"]` parsing — the manifest has `source_of_truth.packed_record_root`).
     3. Copy the `<slug>.jsonl` and `<slug>.index.jsonl` files into the destination.
     4. Reconstruct the packed_manifest with all 12 entries.
     5. Re-run the invariants at lines 1779-1791 against the merged state.
   - Set `CG_MEJEPA_PY_FORWARD_SKIP_EXISTING_PACKED_READ=1` automatically when `CG_MEJEPA_PY_FORWARD_PARALLEL_EMBEDDERS > 1` (transparent default — operators don't need to set both env vars).

4. **Test order (one change at a time per doctrine §1.11):**
   a. Land the Python consolidation merge step alone, keep sequential mode unchanged. Verify sequential regression smoke is byte-equal.
   b. Land the Rust skip-existing flag. Verify a sequential smoke with the flag set produces the same merge-via-Python result. (This is the proof that the Python merge can stand in for the Rust forward-copy.)
   c. Verify parallel=4 smoke succeeds end-to-end.
   d. Verify parallel=6 smoke succeeds and shows the expected speedup.
   e. Tag a 50-shard burn-in test in a separate cache root to catch any subtle accumulation bugs.

5. **Rebuild on aiwonder:**
   - SSH: `croyse@aiwonder.mst.com` via `/tmp/ssh-aiwonder/run-remote.sh` (from operator's WSL).
   - Build env on aiwonder needs `CXXFLAGS="-include cstdint -include cstddef"` for `librocksdb-sys 0.16.0+8.10.0` against GCC 13.4. Cargo cache + target/ dir under `/zfs/hot/contextgraph/repo/target/`.
   - `tmux new-session -d -s contextgraph_build_308 'cd /zfs/hot/contextgraph/repo && . $HOME/.cargo/env && export CXXFLAGS="-include cstdint -include cstddef" && cargo build --release -p context-graph-mejepa-embedders --example python_forward_cache_fsv 2>&1 | tee /zfs/hot/contextgraph/runtime/cargo-build-308.log'`.
   - Expected wall: ~5-10 min for incremental.

6. **Smoke on aiwonder:**
   - Use a fresh task ID like `TASK-PY-DATA-016-AIWONDER-308-FIX-<TS>` to bypass the smoke-gate preflight (which expects WSL-only artifacts).
   - Required env vars (per `docs2/aiwonder.md §4.2`):
     ```
     CG_MEJEPA_DURABLE_ROOT=/zfs/archive/contextgraph
     CG_MEJEPA_DISK_TARGETS=/,/hotpool,/archive
     CG_MEJEPA_HOT_DISK_TARGET=/
     CG_MEJEPA_DURABLE_DISK_TARGET=/archive
     CONTEXTGRAPH_MEJEPA_MODELS_CONFIG=/zfs/hot/contextgraph/models/mejepa_models_config.toml
     CG_MEJEPA_PY_INSTRUMENT_REGISTRY_JSONL=/zfs/hot/contextgraph/corpus/python-frozen-instrument-registry-v1/python_frozen_instrument_records.jsonl
     ```
   - `--cargo-runner /zfs/hot/contextgraph/repo/target/release/examples/python_forward_cache_fsv` (rebuilt binary path).
   - Use `--cache-root` and `--fsv-root` under `/zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-308-fix-<TS>` to keep this isolated from the live prod run.

7. **Verify SoT after each smoke:**
   - `shard-000000-offset-000000000.json` final log present + `exit_code=0`.
   - `cache_record_count = 11880` and `chunk_count = 1024`.
   - All 12 per-embedder run logs `*.eN.json` with `exit_code=0`.
   - `packed_records/` has all 12 `*.jsonl` and `*.index.jsonl` files.
   - `python_forward_cache_packed_manifest.json` has `row_count = 11880`, `per_embedder` keys cover all 12 slugs.

## How to coordinate with the live prod run

- The live prod run uses `--cache-root /zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-prod-20260515-191116Z`. **Do not write anything under this path.**
- The live prod scheduler is running an older version of `python_forward_shard_scheduler.py` (the version synced at prod launch time). Mid-flight updates to the Python file on aiwonder do NOT affect the running scheduler because Python loads it once at startup. So **edits + rsync don't disturb the live run**.
- If the refactor passes all smoke tests, **then** consider:
  1. SIGINT the prod run (it checkpoints + exits cleanly at the shard boundary).
  2. Wait ~1 min for graceful exit.
  3. Relaunch with `--resume-completed-shards` and the new env var `CG_MEJEPA_PY_FORWARD_PARALLEL_EMBEDDERS=6`.
  4. Confirm shard N (where prod stopped) reprocesses cleanly in parallel mode.

But: **do NOT relaunch prod into parallel mode until smoke C (parallel=6) has passed end-to-end at least three times on different shards.** Stability matters more than the 1-day savings if the parallel mode has any subtle bug.

## Doctrine pointers

- `docs2/AICodingAgentSuperPrompt.md` — universal AI coding agent doctrine. Especially §4 (FSV), §3 (GitHub Issues as memory), §11 (forensic discipline).
- `docs2/aiwonder.md` — aiwonder operations rules. §3.2 GPU coexistence policy with leapable-tei (don't touch leapable services), §4.2 scheduler invocation recipe.
- `docs/futurebuild/27_PHASE_G_ROADMAP.md` — overall Phase G plan.
- `CLAUDE.md` — project addendum.

Per the doctrine §3.4: claim issue `#308` before starting code work. Use `gh issue edit 308 --add-label status:in-progress --remove-label status:needs-triage --add-assignee @me` then post a CLAIM comment. Comment milestones (§3.5). Post RESOLVED with FSV evidence when done (§3.8).

## Constraints (hard "never"s)

- **Never break sequential mode.** `CG_MEJEPA_PY_FORWARD_PARALLEL_EMBEDDERS=1` (default) must produce byte-equal output to the pre-refactor sequential path. Verify with sha256 comparison.
- **Never disable MPS** (`nvidia-cuda-mps-control.service` on aiwonder is load-bearing for leapable-tei).
- **Never touch leapable paths** under `/zfs/hot/marketplace/`, `/zfs/hot/postgres/`, `/zfs/hot/redis/`, `/opt/leapable/`, `/etc/leapable/`, `/run/leapable/`.
- **Never write secrets** (Infisical or aiwonder.env values) into the repo, issues, or comments.
- **Never use destructive git** (force push, reset --hard, etc.).
- **Never claim "done" without FSV evidence** at the named SoT paths above.

## Success criteria — when this issue closes

- All five FSV scenarios above pass.
- `cargo test -p context-graph-mejepa-embedders` green.
- `cargo clippy -p context-graph-mejepa-embedders --no-deps --tests --examples -- -D warnings` clean.
- RESOLVED comment on `#308` with: commit SHA, list of files touched, SoT readback values for each FSV scenario, side effects observed, follow-up issues if any.
- `docs2/aiwonder.md §4.2` updated to remove the "parallel mode not yet safe" warning and document the new `CG_MEJEPA_PY_FORWARD_SKIP_EXISTING_PACKED_READ` flag (auto-set in parallel mode).
- Optional: 50-shard burn-in test demonstrating sustained speedup.

## Estimated effort

- Read + LSU: 30-60 min
- Python consolidation merge step: 1-2 hours
- Rust skip-existing flag: 1-2 hours
- Rebuild + 3 smoke tests + verification: 1-2 hours
- Burn-in (optional): 4 hours wall (~15 shards × ~230 s)
- **Total: 4-7 hours of focused work**

Worth it: saves ~1.5 days on the current production run plus all future runs.

---

End of handoff. Start by claiming `#308`, reading `python_forward_cache_fsv.rs` lines 786-800 + 2540-2670 + 2820-2900, and `python_forward_shard_scheduler.py` lines 1652-1791 + 3828-3850. Then form your own conclusion before touching code.
