# TASK-EVAL-015 — Weekly evaluation report writer (mejepa-weekly-report-YYYY-MM-DD.json) + open-research-question tracking

```xml
<task_spec id="TASK-EVAL-015" version="1.0">

<metadata>
  <title>Implement weekly report writer in `crates/context-graph-mejepa/src/eval/weekly_report.rs` — `data/mejepa-eval/mejepa-weekly-report-YYYY-MM-DD.json` (mode 0o600) + open-research-questions seed + drift_severity_history + model_promotion_history + persist-fail fallback</title>
  <status>blocked</status>
  <layer>surface</layer>
  <sequence>15</sequence>
  <implements>
    <item ref="REQ-EVAL-19">Open-research-question tracking — seed 8 entries on first run; carry forward existing entries' mitigation_state on subsequent runs</item>
    <item ref="REQ-EVAL-18">Provenance binding — every report carries corpus_sha, model_weights_sha, calibration_version, constellation_version, code_version, holdout_split_sha</item>
    <item ref="REQ-EVAL-09">drift_severity_history rolling 12-month window from CF_MEJEPA_EVAL_REPORTS</item>
    <item ref="REQ-EVAL-02">model_promotion_history rolling 12-month window from CF_MEJEPA_MODEL_PROMOTIONS</item>
    <item ref="component:WeeklyReportWriter">canonical path `data/mejepa-eval/mejepa-weekly-report-YYYY-MM-DD.json`; mode 0o600; persist-fail fallback to /tmp/</item>
    <item ref="constitution#fsv_protocol">re-read-and-assert before reporting success; mode 0o600 on every persisted file</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EVAL-008</task_ref><!-- check_ship_gate populates ship_gate_passed + ship_gate_failures -->
    <task_ref>TASK-EVAL-011</task_ref><!-- state_transfer + aux_head populated -->
    <task_ref>TASK-EVAL-012</task_ref><!-- Q1/Q2/Q3 + regression_check + shrinkage_applied populated -->
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (weekly_report.rs); 1 modified (runner.rs to wire step 16)</estimated_files>
</metadata>

<context>
This task lands the **canonical weekly artifact** the operator reads to make ship/no-ship decisions. The file path is `data/mejepa-eval/mejepa-weekly-report-YYYY-MM-DD.json` — kept under repo root for reproducibility (per CLAUDE.md §3, NOT scratch under /tmp). Mode 0o600 enforced per SEC-MEJEPA-07. Content is the JSON-serialized `EvalReport` populated by all prior tasks (004 overall, 005 conformal, 006 OOD, 007 Gτ, 008 ship gate, 011 state-transfer + aux-head, 012 Q1/Q2/Q3 + regression + shrinkage); plus rollups this task adds:

1. **Open-research-question carry-forward**: read prior week's `open_research_questions` from CF_MEJEPA_RESEARCH_QUESTIONS via `seed_initial_research_questions` if first run; else carry forward existing `mitigation_state` (Active / Resolved / Deferred) and `evidence_pointers` from CF; update `last_updated = today`.

2. **drift_severity_history**: roll up the last 12 weekly reports' drift severity into `Vec<DriftSeverityEntry>`. Read from CF_MEJEPA_EVAL_REPORTS prefix-scanned by date.

3. **model_promotion_history**: read CF_MEJEPA_MODEL_PROMOTIONS via prefix scan; return last 12 months of promotion entries.

**Persist-fail fallback**: per TECH-EVALUATION pattern, if writing to `data/mejepa-eval/` fails (FS error / disk full / permission), fall back to `/tmp/contextgraph-mejepa-evaluation-fsv/` with the SAME filename and 0o600 mode; log a `MEJEPA_EVAL_REPORT_PERSIST_FAIL` warning.

**Same-week behavior**: if a report for today's date already exists, the new report overwrites it (last-writer-wins per AC-EVAL-01-03 — within-week multiple eval calls produce one canonical "today" file).
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (provenance rule; performance_budgets phase8_ship_gate)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-EVALUATION.md (REQ-EVAL-18, 19; AC-EVAL-01-* weekly report scenarios; AC-EVAL-04-* active-learning rollup)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-EVALUATION.md (write_weekly_report 6-step behavior; carry-forward research questions; drift_severity_history rollup; persist-fail fallback)</file>
  <file purpose="prior_task">TASK-EVAL-008 (ship_gate populated), TASK-EVAL-011 (state-transfer + aux-head), TASK-EVAL-012 (Q1/Q2/Q3 + regression + shrinkage)</file>
</input_context_files>

<prerequisites>
  <check>TASK-EVAL-008, 011, 012 shipped: EvalReport carries ship_gate_passed + state_transfer + Q1/Q2/Q3 + regression_check + shrinkage_applied</check>
  <check>CF_MEJEPA_EVAL_REPORTS, CF_MEJEPA_RESEARCH_QUESTIONS, CF_MEJEPA_MODEL_PROMOTIONS open (TASK-EVAL-003)</check>
  <check>`data/mejepa-eval/` directory writable (created if missing)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/eval/weekly_report.rs` with:
      - `pub fn write_weekly_report(report: &EvalReport, output_dir: &Path, storage: &RocksDbStore) -> Result<PathBuf, EvalError>` per TECH-EVALUATION 6-step behavior:
        1. **Carry-forward research questions**: read from CF_MEJEPA_RESEARCH_QUESTIONS via prefix scan over keys 0x01..0x08; for each `OpenResearchQuestionStatus`, preserve `mitigation_state` and `evidence_pointers`, update `last_updated = today`. If CF empty (first run), call `seed_initial_research_questions()`. Replace `report.open_research_questions` with the carried-forward set; persist back to CF.
        2. **Roll up drift_severity_history**: prefix-scan CF_MEJEPA_EVAL_REPORTS sorted by date descending; deserialize each report; extract `drift_severity_at_report_date`; collect last 52 entries (1 year); store in `report.drift_severity_history`. (When `drift_severity_at_report_date` field doesn't exist on prior reports — pre-Phase-8 reports — treat as `DriftSeverity::Healthy`.)
        3. **Roll up model_promotion_history**: prefix-scan CF_MEJEPA_MODEL_PROMOTIONS sorted by date descending; collect last 52 entries; deserialize via `deserialize_model_promotion`; store in `report.model_promotion_history`.
        4. **Re-validate** report via `EvalReport::try_new(builder)` to ensure all invariants hold post-rollup.
        5. **Compute filename**: `let filename = format!("mejepa-weekly-report-{}.json", report.report_date);` → e.g., `mejepa-weekly-report-2026-05-09.json`
        6. **Persist with fallback**:
           - Primary: `output_dir.join(&filename)` with `OpenOptions::new().create(true).write(true).truncate(true).mode(0o600).open(&path)`
           - On error: warning + retry at `/tmp/contextgraph-mejepa-evaluation-fsv/{filename}` with same mode
           - On second error: `Err(EvalError::ReportPersistFail { cf: "weekly_report_file", backend_error })`
        7. **Persist to CF_MEJEPA_EVAL_REPORTS**: serialize via `serialize_eval_report` and write keyed by `eval_report_key(report.report_date, &Ulid::new())` (a fresh ULID per write — multiple within-week reports keep separate CF rows; the file path is stable so file-on-disk is "the latest within-day" by overwrite, but CF preserves all)
        8. **Re-read-and-assert** the file: open + parse + verify SHA-256 of canonical bytes matches `report.hash_for_determinism()` (sanity check the on-disk JSON deserializes back to byte-equal report)
        9. Return `Ok(canonical_path)`
      - `pub fn load_persisted_research_questions(storage: &RocksDbStore) -> Result<Vec<OpenResearchQuestionStatus>, EvalError>` (helper):
        - Prefix-scan CF_MEJEPA_RESEARCH_QUESTIONS over keys 0x01..0x08; deserialize each via `deserialize_research_question`; sort by `question_id`; if empty, return `seed_initial_research_questions()`.
      - `pub fn persist_research_questions(storage: &RocksDbStore, questions: &[OpenResearchQuestionStatus]) -> Result<(), EvalError>` (helper):
        - For each, key = `research_question_key(q.question_id)`; value = `serialize_research_question(q)`; write to CF.
      - `pub fn rollup_drift_severity_history(storage: &RocksDbStore, weeks: usize) -> Result<Vec<DriftSeverityEntry>, EvalError>` (helper):
        - Prefix-scan CF_MEJEPA_EVAL_REPORTS by date desc; collect last `weeks` entries' (report_date, drift_severity); reverse to ascending; default `DriftSeverity::Healthy` when missing on legacy report.
      - `pub fn rollup_model_promotion_history(storage: &RocksDbStore, weeks: usize) -> Result<Vec<ModelPromotionEntry>, EvalError>` (helper):
        - Prefix-scan CF_MEJEPA_MODEL_PROMOTIONS by date desc; collect last `weeks` entries; reverse to ascending.
    - Update `crates/context-graph-mejepa/src/eval/runner.rs`:
      - Wire step 16 of `run_holdout_eval`: after report is constructed, call `write_weekly_report(&report, &config.output_dir, &storage)` and bind `report_path` for upstream return
    - Add unit + integration tests in `weekly_report.rs` `#[cfg(test)] mod tests`:
      - `weekly_report_writes_to_canonical_path_with_0o600` (Unix only)
      - `weekly_report_filename_format_iso_date` (`mejepa-weekly-report-2026-05-09.json`)
      - `weekly_report_persist_fail_falls_back_to_tmp` (inject FS write error on primary path; verify /tmp fallback)
      - `weekly_report_re_reads_and_validates_canonical_hash` (assert on-disk SHA matches `report.hash_for_determinism()`)
      - `research_questions_seeded_on_first_run` (CF empty → 8 questions inserted)
      - `research_questions_carried_forward_preserve_mitigation_state` (Active stays Active across runs; Resolved stays Resolved)
      - `research_questions_last_updated_advances` (each run bumps `last_updated`)
      - `drift_severity_history_rollup_returns_last_52_weeks`
      - `drift_severity_history_defaults_healthy_for_legacy_reports`
      - `model_promotion_history_rollup_returns_last_52_weeks`
      - `same_day_overwrite_replaces_existing_file` (run twice on same day → single file with second-run content)
      - `cf_eval_reports_keeps_both_within_day_runs` (file is overwritten but CF row count == 2)
  </in_scope>
  <out_of_scope>
    - MCP handler (TASK-EVAL-013 wraps a thin call to `write_weekly_report` after `run_holdout_eval`)
    - CLI subcommand (TASK-EVAL-016)
    - FSV harness body (TASK-EVAL-017)
    - Drift detection logic (Phase 5 SELF-HEALING owns; this task only rolls up the persisted severity)
    - Model promotion logic (Phase 5 owns; this task only rolls up CF_MEJEPA_MODEL_PROMOTIONS)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/eval/weekly_report.rs">
use std::path::{Path, PathBuf};
use super::error::EvalError;
use super::report::EvalReport;
use super::storage::RocksDbStore;
use super::types::{DriftSeverityEntry, ModelPromotionEntry, OpenResearchQuestionStatus};

pub fn write_weekly_report(
    report: &amp;EvalReport,
    output_dir: &amp;Path,
    storage: &amp;RocksDbStore,
) -&gt; Result&lt;PathBuf, EvalError&gt;;

pub fn load_persisted_research_questions(
    storage: &amp;RocksDbStore,
) -&gt; Result&lt;Vec&lt;OpenResearchQuestionStatus&gt;, EvalError&gt;;

pub fn persist_research_questions(
    storage: &amp;RocksDbStore,
    questions: &amp;[OpenResearchQuestionStatus],
) -&gt; Result&lt;(), EvalError&gt;;

pub fn rollup_drift_severity_history(
    storage: &amp;RocksDbStore,
    weeks: usize,
) -&gt; Result&lt;Vec&lt;DriftSeverityEntry&gt;, EvalError&gt;;

pub fn rollup_model_promotion_history(
    storage: &amp;RocksDbStore,
    weeks: usize,
) -&gt; Result&lt;Vec&lt;ModelPromotionEntry&gt;, EvalError&gt;;
    </signature>
  </signatures>
  <constraints>
    - File mode strictly 0o600 on Unix (`std::os::unix::fs::OpenOptionsExt::mode(0o600)`); fallback warning printed on non-Unix per SEC-MEJEPA-07
    - Filename format strictly `mejepa-weekly-report-YYYY-MM-DD.json` (ISO 8601 date)
    - Persist-fail fallback path: `/tmp/contextgraph-mejepa-evaluation-fsv/{filename}` (matches TASK-EVAL-017 FSV evidence dir)
    - Re-read-and-assert step is mandatory per `&lt;fsv_protocol&gt;` — failure surfaces as `EvalError::ReportPersistFail`
    - Research questions: even when carried forward, MUST always have exactly 8 entries Q1..Q8 (validate post-load); if missing entries, fill with `seed_initial_research_questions()` for missing IDs only
    - Drift severity history: when CF empty, return empty Vec (not error)
    - Model promotion history: when CF empty, return empty Vec (not error)
    - Determinism: same input report + same CF state → byte-equal JSON output
    - All Err returns wrapped with `.log_context("eval::weekly_report")`
    - File length budget: &lt; 500 lines
  </constraints>
  <verification>
    - `cargo build/test/clippy` clean
    - ≥ 12 passing tests
    - File written with mode 0o600 (Unix)
    - Re-read SHA matches written report's deterministic hash
    - Research-question carry-forward preserves Active/Resolved/Deferred state
  </verification>
</definition_of_done>

<pseudo_code>
crates/context-graph-mejepa/src/eval/weekly_report.rs:
  pub fn write_weekly_report(report, output_dir, storage) -&gt; Result&lt;PathBuf, EvalError&gt; {
      // 1. carry-forward research questions
      let mut report = report.clone();
      let prior_questions = load_persisted_research_questions(storage)?;
      let mut carried: BTreeMap&lt;ResearchQuestionId, OpenResearchQuestionStatus&gt; = BTreeMap::new();
      for q in prior_questions { carried.insert(q.question_id, q); }
      // ensure all 8 IDs present
      for seed in seed_initial_research_questions() {
          carried.entry(seed.question_id).or_insert(seed);
      }
      // bump last_updated on every active run
      let today = chrono::Utc::now().date_naive();
      for (_, q) in carried.iter_mut() {
          if q.mitigation_state == MitigationState::Active {
              q.last_updated = today;
          }
      }
      report.open_research_questions = carried.into_values().collect();
      persist_research_questions(storage, &report.open_research_questions)?;

      // 2. drift severity history (last 52 weeks)
      report.drift_severity_history = rollup_drift_severity_history(storage, 52)?;

      // 3. model promotion history (last 52 weeks)
      report.model_promotion_history = rollup_model_promotion_history(storage, 52)?;

      // 4. re-validate
      let validated = EvalReport::try_new(EvalReportBuilder::from(report.clone()))?;

      // 5. canonical filename
      let filename = format!("mejepa-weekly-report-{}.json", validated.report_date);
      let primary = output_dir.join(&filename);
      let fallback = std::path::Path::new("/tmp/contextgraph-mejepa-evaluation-fsv/").join(&filename);

      let bytes = validated.serialize_canonical()?;

      // 6. persist with fallback
      let written_path = match write_with_mode_0o600(&primary, &bytes) {
          Ok(()) =&gt; primary.clone(),
          Err(e) =&gt; {
              tracing::warn!(target: "eval::weekly_report", "primary persist fail: {e}; falling back to /tmp/");
              std::fs::create_dir_all(fallback.parent().unwrap()).map_err(|x| EvalError::ReportPersistFail {
                  cf: "weekly_report_dir".into(), backend_error: x.to_string()
              })?;
              write_with_mode_0o600(&fallback, &bytes).map_err(|x| EvalError::ReportPersistFail {
                  cf: "weekly_report_fallback".into(), backend_error: x.to_string()
              })?;
              fallback.clone()
          }
      };

      // 7. CF persist
      let ulid = ulid::Ulid::new();
      let key = eval_report_key(validated.report_date, &ulid);
      let cf_bytes = serialize_eval_report(&validated)?;
      let cf = storage.cf_handle(CF_MEJEPA_EVAL_REPORTS)?;
      storage.put_cf(&cf, &key, &cf_bytes)?;

      // 8. re-read-and-assert
      let on_disk = std::fs::read(&written_path).map_err(|e| EvalError::ReportPersistFail {
          cf: "weekly_report_reread".into(), backend_error: e.to_string()
      })?;
      let parsed: EvalReport = serde_json::from_slice(&on_disk).map_err(|e| EvalError::ReportPersistFail {
          cf: "weekly_report_parse".into(), backend_error: e.to_string()
      })?;
      if parsed.hash_for_determinism() != validated.hash_for_determinism() {
          return Err(EvalError::ReportPersistFail {
              cf: "weekly_report_hash_mismatch".into(),
              backend_error: "on-disk report hash differs from in-memory".into(),
          });
      }

      Ok(written_path)
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/eval/weekly_report.rs">write_weekly_report + 4 helpers + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/eval/runner.rs">wire step 16 of run_holdout_eval to call write_weekly_report and propagate report_path back to caller</file>
  <file path="crates/context-graph-mejepa/src/eval/mod.rs">declare `pub mod weekly_report;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build / test / clippy clean</criterion>
  <criterion>File written at canonical path with mode 0o600 (Unix)</criterion>
  <criterion>Filename matches `mejepa-weekly-report-YYYY-MM-DD.json`</criterion>
  <criterion>Re-read of on-disk file produces byte-equal report (SHA match)</criterion>
  <criterion>Persist-fail fallback to /tmp/ on primary write error</criterion>
  <criterion>Research questions seeded on first run (CF empty → 8 questions)</criterion>
  <criterion>Research-question carry-forward preserves Active/Resolved/Deferred state</criterion>
  <criterion>Research-question last_updated advances on Active entries</criterion>
  <criterion>drift_severity_history rolls up last 52 weeks</criterion>
  <criterion>model_promotion_history rolls up last 52 weeks</criterion>
  <criterion>Same-day re-run overwrites file but CF row count grows</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa eval::weekly_report</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
