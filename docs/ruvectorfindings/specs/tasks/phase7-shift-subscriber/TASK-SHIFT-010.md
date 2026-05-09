<task_spec id="TASK-SHIFT-010" version="1.0">
<metadata>
  <title>Logic — ShiftSubscriber::run Tokio task entry point (1 dedicated CPU + 512 MB RSS)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>10</sequence>
  <implements>
    <item ref="REQ-SHIFT-01">Single Tokio task; 1 CPU + 512 MB RSS budget</item>
    <item ref="REQ-SHIFT-02">Restart hydration from CF_MEJEPA_SHIFT_WATERMARK</item>
    <item ref="REQ-SHIFT-03">Lag-alert evaluation each loop iteration</item>
    <item ref="EC-SHIFT-10">Supervisor catches panics; respawn-or-die per retry count</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#component_contracts[ShiftSubscriber][run]">behaviour</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#restart_recovery_diagram">restart sequence</item>
  </implements>
  <depends_on>
    <task_ref>TASK-SHIFT-005</task_ref>
    <task_ref>TASK-SHIFT-007</task_ref>
    <task_ref>TASK-SHIFT-008</task_ref>
    <task_ref>TASK-SHIFT-009</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file (run.rs) + integration test</estimated_files>
</metadata>

<context>
The Tokio task entry point. Hydrates per-session resume points from `CF_MEJEPA_SHIFT_WATERMARK`; opens tail readers; enters main loop: poll_all → process_shift → write_watermark → evaluate_lag_alert → repeat. Listens for shutdown signal via `tokio::select!`. Supervisor (JoinSet) catches panics inside spawn_blocking encode calls per EC-SHIFT-10.
</context>

<input_context_files>
  <file purpose="component_contract">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#component_contracts[ShiftSubscriber][run]</file>
  <file purpose="restart_diagram">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#restart_recovery_diagram</file>
  <file purpose="implementation_notes">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#implementation_notes (concurrency)</file>
  <file purpose="prior_logic">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-005..009.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-SHIFT-005..009 complete</check>
  <check>`tokio::select!` and `tokio::task::JoinSet` available</check>
</prerequisites>

<scope>
  <in_scope>
    - `pub async fn run(mut self, shutdown: tokio::sync::watch::Receiver<bool>) -> Result<(), SubscriberError>`
    - Startup: validate config, verify all 5 CFs exist, capture code_version, set `task_alive_since`, hydrate watermarks, open readers
    - Main loop: poll_all() → for each parsed entry, await process_shift; on Ok, write_watermark; on Err, log + skip-or-halt (LogParseFail → skip + advance past offset; WatermarkBackwards → halt; transient RocksDb → 3-retry; ObserveBatchNan → still advance watermark per ERR-SHIFT-06 recovery_action: persistence already happened)
    - Per-iteration: recompute lag, call `LagMonitor::evaluate`, on true return emit `tracing::warn!` for the alert
    - Shutdown handling via `tokio::select!`: on shutdown signal, finish in-flight shift, set task_alive_since = None, return Ok
    - Panic supervisor: wrap each `process_shift` call in `tokio::task::JoinSet::spawn`; catch JoinError::is_panic → populate `metrics.last_panic`; respawn up to 3 times on same shift; after 3rd panic on same shift → write `memory/blockers/` entry + return Err
    - Per-session retry counter (HashMap<ShiftId, u8>) cleared on successful process_shift
    - Integration test: spawn run() with 100-shift fixture, watch all consumed, watermark per-session populated, no double-process
    - Integration test: SIGKILL drill (drop the task mid-loop, restart, verify exactly-once on CF rows + at-least-once delivery)
  </in_scope>
  <out_of_scope>
    - MCP server spawning the task (lives in MCP startup wiring, sister phase)
    - Latency export (TASK-SHIFT-012)
    - FSV harness (TASK-SHIFT-013)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-shift-subscriber/src/run.rs">
      use tokio::sync::watch;
      use std::time::Duration;

      impl ShiftSubscriber {
          pub async fn run(
              mut self,
              shutdown: watch::Receiver&lt;bool&gt;,
          ) -&gt; Result&lt;(), SubscriberError&gt;;

          fn record_panic(&amp;self, info: PanicInfo);
          fn current_lag(&amp;self) -&gt; u64;
      }
    </signature>
  </signatures>

  <constraints>
    - Single Tokio task: process_shift is the only async fn awaited per iteration; no `tokio::spawn` of additional sibling tasks
    - Inner-shift parallelism (15-slot buffer_unordered) is fine; outer-shift serialization is the invariant
    - `max_concurrent_shifts: 1` enforced (Q-SHIFT-04 lock; per REQ-SHIFT-17)
    - Watermark advance is the LAST step of a successful shift (after process_shift returns Ok)
    - On `LogParseFail`, advance watermark past the bad byte_offset (so a fixed log can be re-tailed); on `WatermarkBackwards`, halt with the structured error and write a `memory/blockers/` entry
    - Provenance bundle assembled once at startup; embedded in every WatermarkRecord written
    - File length under 500 lines
  </constraints>

  <verification>
    - 100-shift integration test: all 100 consumed in order; final per-session watermarks correct; no double-process
    - SIGKILL drill (test simulates panic mid-process_shift, verifies on respawn the in-flight shift is reprocessed; final CF rows == 100)
    - Panic supervisor test: inject panic in encoder mock, verify last_panic populated, retry count increments, hard-fail after 3
    - Shutdown: signal shutdown → run() returns Ok within 1 in-flight shift's worth of time
  </verification>
</definition_of_done>

<pseudo_code>
run:
  self.config.validate()?;
  // hydrate
  let records = self.watermark_writer.read_all_watermarks()?;
  for rec in &records { self.tail.open_or_create_reader(&rec.session_id, rec.last_consumed_byte_offset).await?; }
  // discover new sessions not yet in watermark CF
  for sid in self.tail.discover_sessions().await? {
    if !records.iter().any(|r| r.session_id == sid) {
      self.tail.open_or_create_reader(&sid, 0).await?;
    }
  }
  *self.metrics.task_alive_since.write() = Some(now_unix_seconds());
  let mut retry_counts: HashMap<ShiftId, u8> = HashMap::new();
  loop {
    tokio::select! {
      biased;
      _ = shutdown.changed() => break,
      results = self.tail.poll_all() => {
        for r in results {
          match r {
            Ok((sid, entry)) => {
              match self.process_shift(entry.clone()).await {
                Ok(_outcome) => {
                  retry_counts.remove(&entry.shift_id);
                  let rec = WatermarkRecord{ session_id: sid, last_consumed_shift_id: entry.shift_id, last_consumed_byte_offset: entry.byte_offset + line_len(&entry), last_advanced_at_unix_seconds: now_unix_seconds(), provenance_corpus_sha: self.provenance.corpus_sha, provenance_code_version: self.provenance.code_version.clone() };
                  self.watermark_writer.write_watermark(&rec)?;
                }
                Err(SubscriberError::WatermarkBackwards{..}) as e => return Err(e),
                Err(SubscriberError::ObserveBatchNan{..}) as e => {
                  tracing::warn!(error_code = e.code(), "observe skipped; persistence already happened");
                  // advance watermark per ERR-SHIFT-06
                  // (same write_watermark as Ok branch)
                }
                Err(e) if let SubscriberError::RocksDbWriteFailed{retry, ..} = &e if *retry < 3 => {
                  // retry next iteration; do NOT advance watermark
                  tracing::warn!(error_code = e.code(), retry = *retry, "transient rocksdb error, retrying");
                }
                Err(e) => return Err(e),
              }
            }
            Err(SubscriberError::LogParseFail{ byte_offset, session_id, .. }) as e => {
              tracing::warn!(error_code = e.code(), "advancing past malformed line");
              // advance to byte_offset + 1 so next poll skips
              // (write watermark with byte_offset advanced; shift_id unchanged from prior good)
            }
            Err(e) => return Err(e),
          }
        }
        // lag eval
        let now = Instant::now();
        if self.lag_monitor.evaluate(&self.metrics, self.current_lag(), now) {
          tracing::warn!(error_code = "MEJEPA_SHIFT_SUBSCRIBER_LAG_ALERT", lag = self.current_lag(), "lag alert active");
        }
      }
    }
  }
  *self.metrics.task_alive_since.write() = None;
  Ok(())
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/run.rs">run() entry point + supervisor logic</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/tests/run_integration.rs">100-shift consumption + SIGKILL drill + panic supervisor</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/lib.rs">`pub mod run;`</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/subscriber.rs">add `pub(crate) lag_monitor: LagMonitor` + `pub(crate) tail: ShiftLogTail` + `pub(crate) watermark_writer: WatermarkWriter` fields</file>
</files_to_modify>

<validation_criteria>
  <criterion>100-shift fixture consumed exactly once</criterion>
  <criterion>SIGKILL drill: 1,000 shifts with 10 random kills → final CF rows == 1,000 (re-processed shifts overwrite atomically)</criterion>
  <criterion>Panic supervisor: 3rd panic on same shift surfaces structured blocker</criterion>
  <criterion>Shutdown signal cleanly terminates run() within one shift's budget</criterion>
  <criterion>WatermarkBackwards halts run()</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-shift-subscriber</command>
  <command>cargo test -p context-graph-mejepa-shift-subscriber run::</command>
  <command>cargo test -p context-graph-mejepa-shift-subscriber --test run_integration</command>
  <command>cargo clippy -p context-graph-mejepa-shift-subscriber -- -D warnings</command>
</test_commands>
</task_spec>
