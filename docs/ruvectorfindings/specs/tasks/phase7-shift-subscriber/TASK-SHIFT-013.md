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

## 📋 2026-05-10 STATE AUDIT (agent-211-task-updater)

**Audit pass against current tree on 2026-05-10:**

- ✅ **Crate `context-graph-mejepa-shift-subscriber` does NOT exist yet** — FSV harness example lands in the same not-yet-existing crate at `examples/shift_subscriber_fsv.rs`.
- ✅ FSV evidence directory `/tmp/contextgraph-mejepa-shift-subscriber-fsv/` does NOT exist yet (no prior FSV run). The harness's `std::fs::create_dir_all` is correct.
- ⚠ **Sister phase evidence dependency:** `mcp-tool-roundtrip-evidence.json` is produced by TASK-MCP-007 (sister phase 7 PENDING). This task explicitly excludes producing that file. TASK-SHIFT-014 (next) asserts on its presence — order matters: MCP roundtrip evidence must be produced first if TASK-SHIFT-014 is run with strict FSV asserts.
- ✅ Test trait impls from TASK-SHIFT-007 (`#[cfg(test)] mod`) are the planned source for predictor/utml/cert/oracle/witness collaborators in the FSV harness.
- ✅ `tempfile` workspace dev-dep available; mode-0o600 enforcement via `std::os::unix::fs::OpenOptionsExt::mode` is the standard pattern.
- No drift in `<files_to_create>` paths.

<task_spec id="TASK-SHIFT-013" version="1.0">
<metadata>
  <title>Surface — FSV harness writing 4 evidence files at /tmp/contextgraph-mejepa-shift-subscriber-fsv/</title>
  <status>ready</status>
  <layer>surface</layer>
  <sequence>13</sequence>
  <implements>
    <item ref="NFR-SHIFT-10">FSV evidence on every release-build CI run</item>
    <item ref="testing_requirements[fsv_evidence_paths]">/tmp/contextgraph-mejepa-shift-subscriber-fsv/{watermark,observe-evidence,predict-latency-evidence,mcp-tool-roundtrip-evidence}.json</item>
    <item ref="REQ-SHIFT-01">FSV evidence at watermark.json captures post-run watermark</item>
    <item ref="REQ-SHIFT-04">predict-latency-evidence.json records p50/p99/cache-hit-rate</item>
    <item ref="REQ-SHIFT-06">observe-evidence.json shows observed + dropped examples</item>
    <item ref="constitution#fsv_protocol">re-read-and-assert before reporting success; mode 0o600</item>
  </implements>
  <depends_on>
    <task_ref>TASK-SHIFT-010</task_ref>
    <task_ref>TASK-SHIFT-011</task_ref>
    <task_ref>TASK-SHIFT-012</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 example bin + 1 fixture-builder helper</estimated_files>
</metadata>

<context>
The FSV harness is a `cargo run --release --example shift_subscriber_fsv` binary that synthesizes a 100-shift JSONL fixture, spawns the subscriber against an in-memory RocksDB, drives consumption to completion, then re-reads the 4 evidence files and runs `assert!` blocks before reporting success. Per constitution `<fsv_protocol>` real data only — no mock embedders, real tree-sitter parses (where applicable), real RocksDB writes. The trait collaborators (predictor, utml, etc.) use the test trait impls from TASK-SHIFT-007 (which compute deterministic outputs from real inputs).

The 5th evidence file `mcp-tool-roundtrip-evidence.json` is produced by the sister phase (TASK-MCP-007) — this task explicitly does NOT produce it.
</context>

<input_context_files>
  <file purpose="fsv_protocol">docs/ruvectorfindings/specs/constitution.md#fsv_protocol</file>
  <file purpose="evidence_paths">docs/ruvectorfindings/specs/constitution.md#testing_requirements[fsv_evidence_paths]</file>
  <file purpose="testing_note">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#implementation_notes (testing row)</file>
  <file purpose="prior_logic_tasks">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-010..012.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-SHIFT-010..012 complete</check>
  <check>Trait test impls available from TASK-SHIFT-007 (predictor/utml/cert/oracle/witness)</check>
  <check>`tempfile` workspace dev-dep available for ephemeral RocksDB instance</check>
</prerequisites>

<scope>
  <in_scope>
    - `crates/context-graph-mejepa-shift-subscriber/examples/shift_subscriber_fsv.rs`
    - Synthetic 100-shift JSONL fixture written to a tempdir's `cgreality-shift-log/<session>.jsonl`
    - Mix: 70 cold + 30 warm (repeat after_sha); 20 oracle-attached (15 with L_step ≥ 0.05, 5 below); 1 malformed line for parse-fail evidence
    - Spawn subscriber with all real components (in-memory RocksDB, real LatencyProfiler, real LagMonitor, test-trait collaborators); shutdown after 100 consumed
    - Write 4 JSON evidence files to `/tmp/contextgraph-mejepa-shift-subscriber-fsv/` with mode 0o600 (per SEC-MEJEPA-07):
      - `watermark.json` — final per-session watermarks + provenance
      - `observe-evidence.json` — 1 observed example + 1 dropped (L_step &lt; 0.05) example + 1 NaN example (forced via mock UTML engine)
      - `predict-latency-evidence.json` — LatencySnapshot + cache hit rate + warm/cold breakdown
      - `mcp-tool-roundtrip-evidence.json` — produced by sister phase TASK-MCP-007 (NOT this task; harness asserts the file's presence as a precondition note)
    - Re-read each written file; assert documented invariants:
      - `watermark.json` records 100 shifts consumed; final shift_id matches synthesized fixture
      - `observe-evidence.json` includes 3 example records with required fields
      - `predict-latency-evidence.json` p50_ms < 5_000 (warm path target); p99_ms < 30_000 (cold path target)
    - Print success summary to stdout
  </in_scope>
  <out_of_scope>
    - MCP roundtrip evidence (TASK-MCP-007)
    - Real predictor weights (test trait impls produce deterministic outputs)
    - Multi-language AST chunker integration (covered by SPEC-AST-CHUNKING FSV)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-shift-subscriber/examples/shift_subscriber_fsv.rs">
      // [example binary]
      #[tokio::main]
      async fn main() -&gt; anyhow::Result&lt;()&gt; {
          let evidence_dir = std::path::Path::new("/tmp/contextgraph-mejepa-shift-subscriber-fsv/");
          std::fs::create_dir_all(evidence_dir)?;
          let fixture = synth_fixture()?;
          let subscriber = build_subscriber_with_test_traits(&amp;fixture).await?;
          let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
          let handle = tokio::spawn(subscriber.run(shutdown_rx));
          wait_for_n_consumed(100, &amp;fixture).await;
          shutdown_tx.send(true)?;
          handle.await??;
          write_watermark_json(evidence_dir, &amp;fixture)?;
          write_observe_evidence(evidence_dir, &amp;fixture)?;
          write_predict_latency_evidence(evidence_dir, &amp;fixture)?;
          assert_re_read_invariants(evidence_dir)?;
          println!("FSV-OK: shift_subscriber");
          Ok(())
      }

      fn synth_fixture() -&gt; anyhow::Result&lt;FsvFixture&gt;;
      fn build_subscriber_with_test_traits(fix: &amp;FsvFixture) -&gt; anyhow::Result&lt;ShiftSubscriber&gt;;
      fn write_watermark_json(dir: &amp;std::path::Path, fix: &amp;FsvFixture) -&gt; anyhow::Result&lt;()&gt;;
      fn write_observe_evidence(dir: &amp;std::path::Path, fix: &amp;FsvFixture) -&gt; anyhow::Result&lt;()&gt;;
      fn write_predict_latency_evidence(dir: &amp;std::path::Path, fix: &amp;FsvFixture) -&gt; anyhow::Result&lt;()&gt;;
      fn assert_re_read_invariants(dir: &amp;std::path::Path) -&gt; anyhow::Result&lt;()&gt;;
    </signature>
  </signatures>

  <constraints>
    - Each evidence file written with mode 0o600 via `std::os::unix::fs::OpenOptionsExt::mode(0o600)` (gated cfg(unix); falls back to default permissions on non-Unix with a warning printed to stderr)
    - Re-read-and-assert is mandatory per `<fsv_protocol>`; the binary fails (non-zero exit) if any assert fails
    - Synthetic ULIDs use a counted SplitMix64 PRNG so the fixture is byte-stable across runs
    - JSON serialization deterministic (sorted keys via `serde_json::to_string_pretty` + `BTreeMap` where applicable)
    - File length under 500 lines
  </constraints>

  <verification>
    - `cargo run --release --example shift_subscriber_fsv -p context-graph-mejepa-shift-subscriber` exits 0
    - All 3 evidence files (this task's responsibility) present; mode 0o600 on Linux/macOS
    - `watermark.json` contains 100 consumed shifts
    - `predict-latency-evidence.json` reports finite p50_ms, p95_ms, p99_ms
    - `observe-evidence.json` contains 3 examples (observed + dropped + nan)
    - Stdout last line == `FSV-OK: shift_subscriber`
  </verification>
</definition_of_done>

<files_to_create>
  <file path="crates/context-graph-mejepa-shift-subscriber/examples/shift_subscriber_fsv.rs">FSV harness binary</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/examples/fsv_fixture.rs">FsvFixture builder + synthetic JSONL writer (shared helper)</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-shift-subscriber/Cargo.toml">add `anyhow` + `tempfile` to [dev-dependencies]; declare both examples in [[example]] tables</file>
</files_to_modify>

<validation_criteria>
  <criterion>FSV binary exits 0 on a clean run</criterion>
  <criterion>3 evidence files created with mode 0o600 on Unix</criterion>
  <criterion>Re-read-and-assert covers each documented invariant</criterion>
  <criterion>Latency p50_ms &lt; 5_000 in warm-cache mode of the fixture</criterion>
  <criterion>Stdout success line matches contract</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release --example shift_subscriber_fsv -p context-graph-mejepa-shift-subscriber</command>
  <command>cargo run --release --example shift_subscriber_fsv -p context-graph-mejepa-shift-subscriber</command>
  <command>ls -la /tmp/contextgraph-mejepa-shift-subscriber-fsv/</command>
</test_commands>
</task_spec>
