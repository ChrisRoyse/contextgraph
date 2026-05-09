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
