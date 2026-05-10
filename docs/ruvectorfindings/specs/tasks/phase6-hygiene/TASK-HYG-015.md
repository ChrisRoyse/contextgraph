# TASK-HYG-015 — gc_steps::step4_prune_checkpoints + tagged-release shield

## 🔍 AI-Agent Implementation Mandate (read FIRST — fresh-context onboarding)

> **You are an AI coding assistant inheriting this task with no prior context. This block gives you everything you need.**

### 1. Project state pointers — read these before touching code

- **Live shipping status table:** `docs/ruvectorfindings/specs/_README.md` Domain map (one row per phase, current as of 2026-05-10).
- **Per-phase deliverable status:** `docs/ruvectorfindings/specs/implementation-plan.md` "Per-phase deliverable table".
- **Constitution (immutable rules):** `docs/ruvectorfindings/specs/constitution.md`.
- **This phase's status snapshot:** the `_index.md` in this same directory.
- **Recent activity:** `git log --oneline -30` shows the last 30 commits.
- **Architecture intent:** `docs/ruvectorfindings/{09,10,11,12,13,14}*.md` (read 09 master plan + 10 pivot first).
- **Project rules:** `CLAUDE.md` at workspace root.

### 2. ⚠ ASSUME THIS TASK IS WRONG — audit before you implement

This task body was authored 2026-05-09. Before implementing, audit it against the current codebase:

1. Do the file paths it references still exist? `find` / `ls` them.
2. Do the type signatures it specifies still match what's in tree? `grep` for the type names in `crates/`.
3. Has the crate it tells you to "create" already been created? Check `crates/`. If it exists, STOP — read what's there, reconcile against the task, and update the task itself before implementing more.
4. Does the spec it cites (e.g., `TECH-<DOMAIN>.md`) still describe the same data shapes? Re-read those specs.

If you find drift, document it in your handoff (`memory/handoff/`) and either (a) update the task to match reality before continuing, or (b) escalate to the operator if scope changed materially.

### 3. Hard rules — ZERO tolerance, fail-closed

1. **No backwards compatibility.** Schema changes wipe + recreate; never write migration shims. Synthetic data only.
2. **No mock data in tests.** Use real types, real corpus rows, real RocksDB CFs, real file SHAs. Tests must FAIL when the system is broken — never write a test that passes against a broken state.
3. **Fail closed with structured error codes** (e.g., `MEJEPA_*`, `UTML_*`). No `unwrap_or(default)` on confidence / quota / correlation / signature / chain-integrity paths. No silent fallback. Every error has `.code()` returning `SCREAMING_SNAKE_CASE`.
4. **Robust error logging.** Every fail-path emits `tracing::error!` with the structured code + actionable context (which file, which CF, which byte offset, which step). When something breaks, the log must say exactly what failed and how to fix it.
5. **No comments unless they encode non-obvious WHY.** Don't explain WHAT — well-named identifiers do that.
6. **Files under 500 lines** (split if exceeded).
7. **Provenance comment** at top of every new `.rs` file: `// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).` Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`.

### 4. Mandatory Full State Verification (FSV) — every task ships with it

Do not rely on return values alone. After the logic runs, you MUST:

1. **Define the Source of Truth.** State explicitly where the final result lives: a RocksDB column family, a `/tmp/contextgraph-*-fsv/*.json` file, a witness-chain segment, a tier-compressed panel blob, a CF watermark row, a tensor allocator's `live_tensors` map, etc.
2. **Execute & Inspect.** Run the logic, then perform a SEPARATE READ on the Source of Truth to verify the data was processed correctly. Cite the readback command (e.g., `cargo run --release -p <crate> --example <fsv_example>` or a direct CF dump).
3. **Boundary & Edge Case Audit.** Manually simulate AT LEAST 3 edge cases — empty inputs, maximum limits, invalid formats, NaN/Inf in floats, control chars in strings, byte offsets at file boundary, etc. For each, print the system state BEFORE and AFTER the action.
4. **Evidence of Success.** Provide a log showing the actual data residing in the system after execution. Cite the log path in the task's `<verification>` block.

### 5. Trigger → outcome verification (mandatory database / artifact check)

> Every triggered event exists to produce a specific intended outcome. The trigger is observable; the outcome is queryable. If your task writes anything to a persistent surface, you MUST query that surface AFTER running and confirm the expected data is there. Do not trust return values alone.

Synthetic protocol: pick an input where you KNOW the expected output. Run it. Manually check the source of truth for the expected output (e.g., open RocksDB and dump the CF, read the FSV JSON, replay the witness chain). Document the trigger → outcome chain in the FSV evidence. Examples:

- Trigger: `compute_l_step({delta_p:0, delta_k:1, delta_omega:1, delta_xi:1})` → Outcome: returns `0.0` exactly (multiplicative gate; any factor at zero zeros the product). FSV: dump training cert, confirm `l_step == 0.0`.
- Trigger: insert `ShiftEntry { shift_id: "01J0123456789ABCDEFGHIJK", ... }` → Outcome: row in `CF_MEJEPA_SHIFT_WATERMARK` with that exact key. FSV: open RocksDB, `cf.get(b"01J0123456789ABCDEFGHIJK")` returns Some.

### 6. Occam's razor

The simplest implementation that satisfies the contract is the correct one. Do NOT add features, abstractions, or fallbacks beyond the task scope. A bug fix doesn't need surrounding cleanup. Three similar lines beats a premature abstraction. If the task says "create struct X with 5 fields", create exactly that — not a builder, not a derive macro family, not a generic over a placeholder.

### 7. Subagent / verification expectations

After implementation:
- Run `cargo build --release -p <crate>` AND `cargo test -p <crate>` AND `cargo clippy -p <crate> -- -D warnings`.
- Spawn the `code-simplifier` subagent (or equivalent) on your changes; apply BLOCKER fixes.
- Write a final SMAP report at `memory/reports/agent-{N}-{role}--final-report.md`.
- Touch the corresponding row in `tasks/_traceability.md` if the REQ→TASK mapping changed.

---

## 📋 2026-05-10 STATE AUDIT — flagged

Task body declares `pub const CF_MEJEPA_WEIGHT_BLOBS: &str = "CF_MEJEPA_WEIGHT_BLOBS";` (line 72). BUT this CF is ALREADY declared in `crates/context-graph-mejepa/src/heal/cf.rs:17`. The hygiene crate must import the existing const via `use context_graph_mejepa::heal::cf::CF_MEJEPA_WEIGHT_BLOBS;` (or move the canonical declaration into a shared `context-graph-mejepa-cf` crate) — DO NOT re-declare with `pub const`. `CF_MEJEPA_WEIGHT_BLOBS_META` (line 73) is NEW and correct to declare here. Adjust the implementation to reflect this.

```xml
<task_spec id="TASK-HYG-015" version="1.0">
<metadata>
  <title>gc_steps::step4_prune_checkpoints — keep last 10 + tagged_release; refuse tagged with MEJEPA_HYG_CHECKPOINT_PRUNE_REFUSED</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>15</sequence>
  <implements>
    <requirement_ref>REQ-HYG-09</requirement_ref>
    <requirement_ref>REQ-HYG-03</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-HYG-003</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
Step 4: list checkpoints in CF_MEJEPA_WEIGHT_BLOBS metadata; sort by created_at desc; keep
newest 10 + all tagged_release=true. Stage deletes in a write_batch (committed only after
step 7 integrity verification per NFR-HYG-04 atomic). Any tagged_release that races into the
prune set is RESPECTED — log WARN with code MEJEPA_HYG_CHECKPOINT_PRUNE_REFUSED; never abort
the step (per BR-HYG-05).
</context>

<input_context_files>
  <file purpose="step_spec">../../technical/TECH-HYGIENE.md#component_contracts</file>
  <file purpose="business_rule">../../functional/SPEC-HYGIENE.md#business_rules</file>
  <file purpose="error_state">../../functional/SPEC-HYGIENE.md#error_states</file>
</input_context_files>

<prerequisites>
  <check>TASK-HYG-003 shipped</check>
</prerequisites>

<scope>
  <in_scope>
    - CREATE `src/gc_steps/step4_prune_checkpoints.rs`:
      * `pub struct CheckpointMeta { pub theta_sha: [u8;32], pub created_at_ts: u64, pub tagged_release: bool, pub tagged_at_ts: Option<u64>, pub size_bytes: u64 }`
      * `pub fn run(env: &HygieneEnv, report: &mut GcReport, staged_batch: &mut WriteBatch)
        -> OpsResult<()>`:
        1. List all checkpoints from CF_MEJEPA_WEIGHT_BLOBS_META; deserialize CheckpointMeta.
        2. Sort by created_at_ts descending.
        3. Determine prune set: skip newest 10; skip all tagged_release=true.
        4. For each prune candidate:
           - If tagged_release==true (defensive double-check):
             append "MEJEPA_HYG_CHECKPOINT_PRUNE_REFUSED" to errors_encountered;
             tracing::warn!(target:"hygiene", checkpoint_id, theta_sha, tagged_at_ts);
             continue (do not stage delete).
           - Else: stage delete in `staged_batch` for both blob CF and meta CF.
             Sum size_bytes into bytes_freed.
        5. Build GcStepReport; push.
    - The `staged_batch` is owned by the orchestrator (TASK-HYG-020); step 4 + step 5
      append to it; step 7 commits or drops it.
    - Unit / integration tests:
      * 20 checkpoints (5 tagged, 15 untagged): 10 newest + 5 tagged kept (15); 5 oldest untagged staged.
      * If a tagged_release somehow appears in the prune set (test injection): error logged; step continues.
      * 0 checkpoints: bytes_freed=0; entries_processed=0.
  </in_scope>
  <out_of_scope>
    - Tagging API (operator runbook out of scope per spec).
    - Step 7 commit (TASK-HYG-018).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-hygiene/src/gc_steps/step4_prune_checkpoints.rs">
    pub const CF_MEJEPA_WEIGHT_BLOBS: &amp;str = "CF_MEJEPA_WEIGHT_BLOBS";
    pub const CF_MEJEPA_WEIGHT_BLOBS_META: &amp;str = "CF_MEJEPA_WEIGHT_BLOBS_META";

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct CheckpointMeta {
        pub theta_sha: [u8; 32],
        pub created_at_ts: u64,
        pub tagged_release: bool,
        pub tagged_at_ts: Option&lt;u64&gt;,
        pub size_bytes: u64,
    }

    pub fn run(
        env: &amp;crate::HygieneEnv,
        report: &amp;mut crate::reports::GcReport,
        staged_batch: &amp;mut rocksdb::WriteBatch,
    ) -&gt; crate::error::OpsResult&lt;()&gt;;
    </signature>
  </signatures>

  <constraints>
    - tagged_release is checked twice: once in candidate filter, once defensively per-entry
      (race-safe per BR-HYG-05).
    - Refused tagged prune does NOT abort the step; just logs + appends to errors_encountered.
    - Stages deletes in caller's WriteBatch; does not commit on its own.
    - File ≤ 500 lines.
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-mejepa-hygiene` succeeds.
    - `cargo test -p context-graph-mejepa-hygiene -- step4_prune_keeps_10_plus_tagged` passes.
    - `cargo test -p context-graph-mejepa-hygiene -- step4_prune_refuses_tagged_continues` passes.
    - clippy clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub fn run(env: &amp;HygieneEnv, report: &amp;mut GcReport, batch: &amp;mut WriteBatch) -&gt; OpsResult&lt;()&gt; {
    let started = std::time::Instant::now();
    let cf_meta = env.db.cf_handle(CF_MEJEPA_WEIGHT_BLOBS_META)
        .ok_or_else(|| OpsError::internal("ckpt_meta_cf_missing"))?;
    let cf_blob = env.db.cf_handle(CF_MEJEPA_WEIGHT_BLOBS)
        .ok_or_else(|| OpsError::internal("ckpt_blob_cf_missing"))?;
    let mut all = read_all_checkpoints(env, &amp;cf_meta)?;
    all.sort_by_key(|m| std::cmp::Reverse(m.created_at_ts));
    let mut bytes_freed = 0i64;
    let mut errs = Vec::new();
    let mut entries_processed = 0u64;
    for (i, ckpt) in all.iter().enumerate() {
        if i &lt; 10 || ckpt.tagged_release { continue; }
        // Defensive double-check (race-safe)
        if ckpt.tagged_release {
            errs.push("MEJEPA_HYG_CHECKPOINT_PRUNE_REFUSED".into());
            tracing::warn!(target:"hygiene", theta_sha=?hex::encode(ckpt.theta_sha),
                tagged_at_ts=?ckpt.tagged_at_ts, "refused tagged-release prune");
            continue;
        }
        batch.delete_cf(&amp;cf_blob, &amp;ckpt.theta_sha);
        batch.delete_cf(&amp;cf_meta, &amp;ckpt.theta_sha);
        bytes_freed += ckpt.size_bytes as i64;
        entries_processed += 1;
    }
    report.step_reports.push(GcStepReport { step_index: 4, step_name: "prune_checkpoints".into(),
        duration_ms: started.elapsed().as_millis() as u64,
        entries_processed, bytes_freed, errors_encountered: errs });
    Ok(())
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-hygiene/src/gc_steps/step4_prune_checkpoints.rs">checkpoint pruning + tagged-release shield</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-hygiene/src/gc_steps/mod.rs">declare step4</file>
</files_to_modify>

<validation_criteria>
  <criterion>20 ckpts (5 tagged, 15 untagged): 10 newest + 5 tagged kept</criterion>
  <criterion>Tagged-release double-check: refused logs warn + step continues</criterion>
  <criterion>Empty checkpoint list: zero counts; no error</criterion>
  <criterion>Deletes are STAGED in batch (not yet committed)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-hygiene</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- step4_prune_keeps_10_plus_tagged</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- step4_prune_refuses_tagged_continues</command>
  <command>cargo clippy -p context-graph-mejepa-hygiene -- -D warnings</command>
</test_commands>
</task_spec>
```
