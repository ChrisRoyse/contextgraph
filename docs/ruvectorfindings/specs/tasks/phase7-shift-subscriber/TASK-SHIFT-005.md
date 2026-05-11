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

## 📋 2026-05-11 STATE AUDIT (sharpened against verified codebase + 2026 best practices)

> **Codebase reality table:** see `_index.md` in this directory.

**Confirmed 2026-05-11:**
- Producer side: `crates/context-graph-mcp/src/handlers/tools/reality_loop/shift_log.rs::append_shift()` (line 71) writes line-by-line `serde_json::to_string(&ShiftRecord) + "\n"`, calls `file.sync_all()` after each write. So every shift hits disk durably before the writer returns.
- Default runtime root: `file:/mnt/d/contextgraph/state/cgreality/active_runtime_root` per the latest D-mount migration. Tests use `tempfile::TempDir`.

**2026 best-practice refresh (per WebSearch):**
- File-tail rotation detection MUST combine: (1) inode comparison (Unix `BasicFileAttributes.fileKey()`-equivalent via `MetadataExt::ino()`), (2) creation-time guard (ext4 inode reuse trap — different `ctime` ⇒ treat as new file even if inode matches), (3) size monotonicity (offset > current file_size ⇒ copytruncate rotation, drain the OLD file before switching).
- Use `tokio::fs::File` + `tokio::io::BufReader` + `read_line()` with a poll-interval `tokio::time::sleep` between EOF-empty reads (see Tokio "infinite data streams" pattern).
- Half-written line (no `\n` at EOF) ⇒ return None, retry on next poll. NEVER parse a partial line.

**Decided scope for this task (refined):**
1. `ShiftLogTail::open_or_create_reader(session_id, start_byte_offset)` opens `<root>/cgreality-shift-log/<session_hex>.jsonl`, seeks to `start_byte_offset`, captures `(inode, ctime, size)` for the rotation detector.
2. `poll_next_line(session_id) -> Result<Option<(String, u64)>, SubscriberError>`: read_line with `tokio::time::timeout(poll_interval)`; on rotation detection (inode change OR ctime change OR `offset > file.metadata()?.len()`), close the OLD reader, drain remaining bytes, open the new file at offset 0, retry once.
3. `discover_sessions()`: read directory; filter `*.jsonl`; reject filenames containing control chars or path separators per SEC-MEJEPA-01.
4. NEVER block the runtime — every read goes through `tokio::fs` (NOT `std::fs`).

**FSV for this task:** integration test with synthetic JSONL (a) writes 100 lines slowly while tail reads them → asserts every line consumed in order; (b) rotates the file mid-tail (delete + recreate) → asserts no shift dropped, no shift duplicated; (c) writes a half-line, waits, completes the line → asserts tail returns the line only once it's complete. Source of Truth: in-memory consumer log of `(line_hash, byte_offset)` tuples vs known producer-side tuples.

---

<task_spec id="TASK-SHIFT-005" version="1.0">
<metadata>
  <title>Logic — ShiftLogTail line-buffered reader + rotation detector + resume-from-offset</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>5</sequence>
  <implements>
    <item ref="REQ-SHIFT-01">Tails per-session JSONL files</item>
    <item ref="REQ-SHIFT-02">Resume from last_consumed_byte_offset on restart</item>
    <item ref="EC-SHIFT-01">File rotation mid-run</item>
    <item ref="EC-SHIFT-02">Half-written line waits for newline</item>
    <item ref="AC-SHIFT-01-04">Malformed JSONL → MEJEPA_SHIFT_SUBSCRIBER_LOG_PARSE_FAIL with byte_offset</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#component_contracts[ShiftLogTail]">5 methods</item>
    <item ref="NFR-SHIFT-09">Path canonicalization at boundary</item>
  </implements>
  <depends_on>
    <task_ref>TASK-SHIFT-001</task_ref>
    <task_ref>TASK-SHIFT-002</task_ref>
    <task_ref>TASK-SHIFT-003</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file (tail.rs body) + tests</estimated_files>
</metadata>

<context>
Implements the asynchronous tail reader that feeds the shift subscriber. Reads `/tmp/contextgraph-rust-reality-loop/cgreality-shift-log/<session_id>.jsonl` line-by-line via `tokio::fs::File + BufReader`, resumes from any byte offset, and detects file rotation (inode/size monotonicity). Half-written lines (no `\n`) yield `Ok(None)` so the caller retries on next poll. The watermark advance still happens in TASK-SHIFT-008 / TASK-SHIFT-010; this task is purely the producer side of the byte-offset → ShiftEntry pipeline.
</context>

<input_context_files>
  <file purpose="component_contract">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#component_contracts[ShiftLogTail]</file>
  <file purpose="rotation_detection_diagram">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#restart_recovery_diagram</file>
  <file purpose="prior_skeleton">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-002.md (tail.rs skeleton)</file>
  <file purpose="error_taxonomy">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-003.md</file>
  <file purpose="existing_jsonl_producer">crates/context-graph-mcp/src/handlers/tools/reality_loop/shift_log.rs (schema reference)</file>
</input_context_files>

<prerequisites>
  <check>TASK-SHIFT-001..003 complete</check>
  <check>`tokio` workspace dep with `fs` + `io-util` features</check>
</prerequisites>

<scope>
  <in_scope>
    - Implement `ShiftLogTail::new`, `open_or_create_reader`, `poll_next_line`, `poll_all`, `discover_sessions`
    - Inode capture via `std::os::unix::fs::MetadataExt::ino()` (Linux/macOS) gated on `cfg(unix)`
    - Rotation detection: if inode changes OR new file size < seek position, reopen at offset 0 and emit rotation log entry
    - Path canonicalization: reject control chars in filenames; reject symlinks pointing outside `root_dir`
    - Half-written line handling: `read_line` returns 0 bytes OR line without trailing `\n` → return `Ok(None)`; caller pauses ≤ `poll_interval`
    - Parse JSONL line via `serde_json::from_str::<ShiftEntry>(line)`; on failure return `SubscriberError::LogParseFail` with byte_offset, sanitized 200-char preview, parse_error string
    - Tests: synthetic JSONL fixture with 100 valid + 1 malformed + 1 half-written line; verify exactly 100 successful parses + 1 LogParseFail + 1 retry-on-next-poll
    - Tests: rotation drill (write 50 lines, truncate file, write new content, verify reopen)
  </in_scope>
  <out_of_scope>
    - Watermark advance (TASK-SHIFT-008)
    - Lag computation (TASK-SHIFT-009)
    - Calling `process_shift` (TASK-SHIFT-010)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-shift-subscriber/src/tail.rs">
      use crate::entry::{SessionId, ShiftEntry};
      use crate::error::SubscriberError;
      use std::path::PathBuf;
      use std::time::Duration;
      use tokio::io::AsyncBufReadExt;

      impl ShiftLogTail {
          pub fn new(root_dir: PathBuf, poll_interval: Duration) -&gt; Self;

          pub async fn open_or_create_reader(
              &amp;mut self,
              session_id: &amp;SessionId,
              start_byte_offset: u64,
          ) -&gt; Result&lt;(), SubscriberError&gt;;

          // Returns:
          //   Ok(Some((line, byte_offset_of_start)))  — full line read
          //   Ok(None)                                  — half-written / EOF; caller retries
          //   Err(...)                                  — I/O or rotation-after-failure
          pub async fn poll_next_line(
              &amp;mut self,
              session_id: &amp;SessionId,
          ) -&gt; Result&lt;Option&lt;(String, u64)&gt;, SubscriberError&gt;;

          // Round-robin across all open readers; parses via serde_json::from_str.
          // On parse fail returns Err(SubscriberError::LogParseFail) for that one line; caller decides advance policy.
          pub async fn poll_all(&amp;mut self) -&gt; Vec&lt;Result&lt;(SessionId, ShiftEntry), SubscriberError&gt;&gt;;

          pub async fn discover_sessions(&amp;self) -&gt; Result&lt;Vec&lt;SessionId&gt;, SubscriberError&gt;;

          fn detect_rotation(
              &amp;mut self,
              session_id: &amp;SessionId,
          ) -&gt; Result&lt;bool, SubscriberError&gt;;
      }
    </signature>
  </signatures>

  <constraints>
    - Use `tokio::fs::OpenOptions::new().read(true).open(path)` with explicit `seek(SeekFrom::Start(offset))`
    - `BufReader` capacity 64 KiB; never load whole file into memory
    - Path canonicalization at `discover_sessions` AND at `open_or_create_reader` (defense-in-depth)
    - Filename validation: must match `^[A-Za-z0-9_-]+\.jsonl$`; reject control chars; reject `..`
    - `poll_next_line` returning Ok(None) MUST NOT block longer than `poll_interval`
    - On rotation detection, log a structured event with old/new inode + size; reopen at offset 0 and continue (no shift drop because watermark is per-`(session_id, shift_id)`)
    - Sanitized line preview: replace control chars with `?`; truncate to first 200 chars (`floor_char_boundary` for UTF-8)
    - File length under 500 lines
  </constraints>

  <verification>
    - Test: 100 well-formed JSONL shifts → 100 successful `poll_all` results
    - Test: insert 1 malformed line at byte_offset 5000 → `LogParseFail { byte_offset: 5000, ... }`
    - Test: write half a line then close test fixture → first poll returns `Ok(None)`; complete the line; next poll returns `Ok(Some(_))`
    - Test: rotate file (truncate + rewrite) → tail detects rotation, reopens, resumes
    - Test: filename with control char rejected by `discover_sessions`
    - `cargo test -p context-graph-mejepa-shift-subscriber tail::tests` all pass
  </verification>
</definition_of_done>

<pseudo_code>
poll_next_line:
  let reader = self.readers.get_mut(session_id).ok_or(SubscriberError::NoReader)?;
  let line_start = reader.position();           -- track via wrapping seek
  let mut line = String::new();
  match tokio::time::timeout(self.poll_interval, reader.read_line(&mut line)).await {
    Ok(Ok(0))            => return Ok(None),    -- EOF
    Ok(Ok(_)) if !line.ends_with('\n') => {
      reader.seek(line_start)?;                 -- re-position; half-written
      return Ok(None);
    }
    Ok(Ok(_))            => return Ok(Some((line.trim_end().to_owned(), line_start))),
    Ok(Err(e))           => return Err(SubscriberError::from(e)),
    Err(_timeout)        => return Ok(None),
  }

detect_rotation:
  let meta = std::fs::metadata(path)?;
  let new_inode = meta.ino();
  let new_size  = meta.len();
  let old_inode = self.rotation_detector.inode_by_session.get(session_id);
  let old_size  = self.rotation_detector.size_by_session.get(session_id);
  if old_inode != Some(new_inode) || (old_size.unwrap_or(0) > new_size) { Ok(true) } else { Ok(false) }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-shift-subscriber/tests/tail_integration.rs">100-shift fixture + malformed line + rotation drill</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/tail.rs">implement 5 methods + private detect_rotation</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/Cargo.toml">enable tokio fs+io-util features; add `tempfile` dev-dep for fixtures</file>
</files_to_modify>

<validation_criteria>
  <criterion>100-shift fixture round-trips with zero parse errors</criterion>
  <criterion>Malformed JSONL surfaces LogParseFail with correct byte_offset</criterion>
  <criterion>Half-written line returns Ok(None); completes successfully on retry</criterion>
  <criterion>Rotation reopens file at offset 0 and resumes</criterion>
  <criterion>Path traversal payloads (`../etc/passwd`) rejected by discover_sessions</criterion>
  <criterion>p50 poll_next_line latency &lt; 10 ms on warm reader (verified by latency histogram in test)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-shift-subscriber</command>
  <command>cargo test -p context-graph-mejepa-shift-subscriber tail::</command>
  <command>cargo test -p context-graph-mejepa-shift-subscriber --test tail_integration</command>
  <command>cargo clippy -p context-graph-mejepa-shift-subscriber -- -D warnings</command>
</test_commands>
</task_spec>
