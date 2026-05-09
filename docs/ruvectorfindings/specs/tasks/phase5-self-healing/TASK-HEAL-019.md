# TASK-HEAL-019 — FSV harness (5 evidence files at `/tmp/contextgraph-mejepa-self-healing-fsv/`, mode 0o600)

```xml
<task_spec id="TASK-HEAL-019" version="1.0">

<metadata>
  <title>Phase 5 master FSV evidence harness in `crates/context-graph-mejepa/examples/phase5_self_healing_fsv.rs`</title>
  <status>blocked</status>
  <layer>surface</layer>
  <sequence>19</sequence>
  <implements>
    <item ref="REQ-HEAL-15">FSV evidence: 5 files at /tmp/contextgraph-mejepa-self-healing-fsv/ (drift-drill, abc-promotion, lora-refresh, witness-integrity, ewc-fisher) — all mode 0o600 per SEC-MEJEPA-07</item>
    <item ref="REQ-HEAL-16">Real-data tests only (no mocks)</item>
    <item ref="AC-HEAL-15">Phase 5 DoD ship-gate: all 5 FSV files exist + 0o600 mode + drill exit 0</item>
    <item ref="SEC-MEJEPA-07">All FSV evidence files mode 0o600 per security_requirements</item>
  </implements>
  <depends_on>
    <task_ref>TASK-HEAL-006</task_ref>
    <task_ref>TASK-HEAL-009</task_ref>
    <task_ref>TASK-HEAL-013</task_ref>
    <task_ref>TASK-HEAL-014</task_ref>
    <task_ref>TASK-HEAL-018</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (examples/phase5_self_healing_fsv.rs); 1 created (src/heal/fsv.rs canonical writer); 1 modified (Cargo.toml [[example]] + dev-dependencies)</estimated_files>
</metadata>

<context>
Surface 4 of 4. The Phase 5 master FSV harness — pulls together the evidence emitted by TASK-HEAL-018
plus the per-primitive `*Evidence` structs (TASK-HEAL-009 AbcFsvEvidence, TASK-HEAL-013
IntegrityEvidence, TASK-HEAL-014 LoraRefreshEvidence, plus EwcFisherEvidence, DriftDrillEvidence
defined here) into the canonical 5 files at `/tmp/contextgraph-mejepa-self-healing-fsv/`. Differs
from TASK-HEAL-018 in role: TASK-HEAL-018 is the EXECUTABLE drill that drives the pipeline; this
task is the EVIDENCE FORMATTER + permissions-enforcer + ship-gate VERIFIER. The example binary
re-runs the drill (or reads its existing output) and produces the canonical FSV directory whose
existence + 0o600-mode + JSON-schema-validity is the Phase 5 ship-gate.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (`<security_requirements>` SEC-MEJEPA-07 mode 0o600; `<failure_taxonomy>`; CLAUDE.md §11 real-data-only)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-SELF-HEALING.md (REQ-HEAL-15 FSV evidence list + AC-HEAL-15 ship-gate)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-SELF-HEALING.md (component_contracts HealDrillCli step 9 file modes; data_models AbcFsvEvidence etc.)</file>
  <file purpose="prior_task">TASK-HEAL-006 EwcPlusPlus (Fisher full-rank assertion; lambda trajectory)</file>
  <file purpose="prior_task">TASK-HEAL-009 AbcPromoter (AbcFsvEvidence struct emission)</file>
  <file purpose="prior_task">TASK-HEAL-013 ChainIntegrityChecker (IntegrityEvidence struct emission)</file>
  <file purpose="prior_task">TASK-HEAL-014 LoraRefresher (LoraRefreshEvidence struct emission + frozen-base byte-equal invariant assertion)</file>
  <file purpose="prior_task">TASK-HEAL-018 (mejepa-heal-drill binary that produces the raw output this task formalizes)</file>
  <file purpose="example_pattern">crates/context-graph-mejepa-instruments/examples/phase1_extended_fsv.rs (existing FSV example pattern; the prior phase's harness)</file>
</input_context_files>

<prerequisites>
  <check>TASK-HEAL-006/009/013/014/018 complete</check>
  <check>Each per-primitive `*Evidence` struct is `Serialize + Deserialize + PartialEq` per data_models contracts</check>
  <check>TASK-HEAL-018 exposes a programmatic entry: `pub fn run_heal_drill(args: HealDrillArgs) -> Result<DrillStdoutSummary, HealError>` (in addition to the binary `fn main`) so this example can drive it without `Command::new`</check>
  <check>FSV directory `/tmp/contextgraph-mejepa-self-healing-fsv/` is writable (0o755 created if absent; files within mode 0o600)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/heal/fsv.rs`:
      - `pub const FSV_ROOT: &str = "/tmp/contextgraph-mejepa-self-healing-fsv";`
      - `pub const FSV_FILES: &[&str] = &["drift-drill-evidence.json", "abc-promotion-evidence.json", "lora-refresh-evidence.json", "witness-integrity-evidence.json", "ewc-fisher-evidence.json"];`
      - `pub fn write_fsv_evidence_canonical<T: Serialize>(file_name: &str, payload: &T, root: Option<&Path>) -> Result<PathBuf, HealError>` — writes payload as pretty-JSON to `<root>/<file_name>`; sets mode 0o600; returns absolute PathBuf; root defaults to FSV_ROOT
      - `pub fn assert_fsv_directory_complete(root: Option<&Path>) -> Result<(), HealError>` — asserts ALL 5 files in FSV_FILES exist + each is mode 0o600 + each parses as valid JSON; returns Err(HealError::FsvIncomplete { missing: Vec<String>, wrong_mode: Vec<(String, u32)>, parse_failures: Vec<String> }) on any failure
      - `pub fn fsv_directory_summary(root: Option<&Path>) -> Result<FsvDirectorySummary, HealError>` — returns `{ root_absolute, file_paths_present, file_modes, file_sizes_bytes, total_bytes, all_complete: bool }`
      - `pub struct DriftDrillEvidence { inject_drift: InjectDrift, observations_to_detect: u64, severity_history: Vec<(u64, DriftSeverity)>, empirical_coverage_trajectory: Vec<f32>, max_observations: u64, hit_max_without_detection: bool }`
      - `pub struct EwcFisherEvidence { boundary_step_trajectory: Vec<u64>, lambda_trajectory: Vec<f32>, fisher_rank_per_boundary: Vec<usize>, fisher_full_rank_after_boundary_10: bool, task_snapshots_count: usize, predictor_dim: usize }`
      - `pub fn append_fsv_master_index(root: Option<&Path>) -> Result<(), HealError>` — writes `<root>/_master_index.json` (mode 0o600) with: `{ phase: "phase5-self-healing", commit_sha, generated_at, file_paths, gate_summary: { drift_detected, mode_winner, lora_succeeded, integrity_caught, fisher_full_rank, all_modes_0o600 } }` — informational; not one of the 5 ship-gate files but useful for code review
    - Create `crates/context-graph-mejepa/examples/phase5_self_healing_fsv.rs`:
      - `fn main() -> Result<(), HealError>` body:
        1. Parse args: `--inject-drift {hard}` (default hard for ship-gate; soft + catastrophic also accepted for additional runs), `--output-root {PATH}` (default FSV_ROOT), `--seed {u64}` (default 42)
        2. Ensure FSV root directory exists: `fs::create_dir_all(root)` (chmod 0o755)
        3. Invoke `mejepa_heal_drill::run_heal_drill(HealDrillArgs { ... })` programmatically (NOT via Command::new — keep within-process for stack traces)
        4. Read the 5 FSV files written by run_heal_drill (already mode 0o600 per TASK-HEAL-018 step 9)
        5. Re-validate: parse each JSON; assert schema-conformity (every required field present; no NaN/Inf); call `assert_fsv_directory_complete(Some(root))?`
        6. Generate `_master_index.json` via `append_fsv_master_index(Some(root))`
        7. Print stdout JSON: `FsvHarnessReport { drill_summary: DrillStdoutSummary, fsv_directory_summary: FsvDirectorySummary, ship_gate_pass: bool, all_5_files_present: bool, all_modes_0o600: bool, all_files_parse_as_json: bool, total_evidence_bytes: usize }`
        8. Exit 0 if `ship_gate_pass`; exit 1 otherwise
      - Per soft/hard/catastrophic runs (operator may invoke 3 times); each subsequent run REPLACES (not appends to) the FSV files (cleanup before write per `<failure_logging>` real-time semantics)
    - Tests:
      - `write_fsv_evidence_canonical_sets_mode_0o600`
      - `write_fsv_evidence_canonical_creates_root_dir_if_absent`
      - `write_fsv_evidence_canonical_overwrites_existing_file`
      - `write_fsv_evidence_canonical_pretty_json_format` (assert indent + newline)
      - `assert_fsv_directory_complete_passes_when_all_5_files_0o600_and_valid_json`
      - `assert_fsv_directory_complete_fails_when_one_file_missing`
      - `assert_fsv_directory_complete_fails_when_one_file_mode_644` (wrong mode)
      - `assert_fsv_directory_complete_fails_when_one_file_invalid_json`
      - `fsv_directory_summary_returns_total_bytes`
      - `append_fsv_master_index_writes_index_json_with_commit_sha` (read commit_sha via env CARGO_PKG_VERSION + git rev-parse fallback)
      - `drift_drill_evidence_round_trip_serde` (Deserialize + Serialize byte-equal)
      - `ewc_fisher_evidence_round_trip_serde`
      - `fsv_root_const_starts_with_tmp_contextgraph` (sanity: FSV_ROOT starts with /tmp/contextgraph)
      - `fsv_files_const_has_exactly_5_entries` (the 5-file ship-gate count)
      - End-to-end: run example with `--inject-drift hard`; assert exit 0 + 5 files at FSV_ROOT + master index + all modes 0o600
      - End-to-end with `--inject-drift catastrophic`: assert exit 0 (catastrophic is also a valid Phase 5 path: full retrain triggered)
      - End-to-end with `--inject-drift soft`: assert exit 0 (no A/B/C triggered; mode_winner remains A_unchanged but FSV files still emitted with that fact recorded)
    - Ship-gate verification matrix written to `_master_index.json` (informational; the binary REASON files are the 5 listed in FSV_FILES)
  </in_scope>
  <out_of_scope>
    - The drill itself (TASK-HEAL-018 is the executable; this task is the formatter + verifier)
    - The per-primitive Evidence struct definitions (TASK-HEAL-009/013/014 already define their own; this task only re-uses + persists them)
    - Production deployment / CI integration (the binary IS the verification; CI invokes it)
    - The MCP tool surfaces (TASK-HEAL-016 + TASK-HEAL-017)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/heal/fsv.rs">
use std::path::{Path, PathBuf};
use serde::Serialize;
use crate::heal::errors::HealError;

pub const FSV_ROOT: &amp;str = "/tmp/contextgraph-mejepa-self-healing-fsv";
pub const FSV_FILES: &amp;[&amp;str] = &amp;[
    "drift-drill-evidence.json",
    "abc-promotion-evidence.json",
    "lora-refresh-evidence.json",
    "witness-integrity-evidence.json",
    "ewc-fisher-evidence.json",
];

pub fn write_fsv_evidence_canonical&lt;T: Serialize&gt;(
    file_name: &amp;str,
    payload: &amp;T,
    root: Option&lt;&amp;Path&gt;,
) -&gt; Result&lt;PathBuf, HealError&gt;;

pub fn assert_fsv_directory_complete(root: Option&lt;&amp;Path&gt;) -&gt; Result&lt;(), HealError&gt;;

pub fn fsv_directory_summary(root: Option&lt;&amp;Path&gt;) -&gt; Result&lt;FsvDirectorySummary, HealError&gt;;

pub fn append_fsv_master_index(root: Option&lt;&amp;Path&gt;) -&gt; Result&lt;(), HealError&gt;;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FsvDirectorySummary {
    pub root_absolute: PathBuf,
    pub file_paths_present: Vec&lt;PathBuf&gt;,
    pub file_modes: Vec&lt;u32&gt;,
    pub file_sizes_bytes: Vec&lt;u64&gt;,
    pub total_bytes: u64,
    pub all_complete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DriftDrillEvidence {
    pub inject_drift: InjectDrift,
    pub observations_to_detect: u64,
    pub severity_history: Vec&lt;(u64, DriftSeverity)&gt;,
    pub empirical_coverage_trajectory: Vec&lt;f32&gt;,
    pub max_observations: u64,
    pub hit_max_without_detection: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EwcFisherEvidence {
    pub boundary_step_trajectory: Vec&lt;u64&gt;,
    pub lambda_trajectory: Vec&lt;f32&gt;,
    pub fisher_rank_per_boundary: Vec&lt;usize&gt;,
    pub fisher_full_rank_after_boundary_10: bool,
    pub task_snapshots_count: usize,
    pub predictor_dim: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FsvHarnessReport {
    pub drill_summary: DrillStdoutSummary,
    pub fsv_directory_summary: FsvDirectorySummary,
    pub ship_gate_pass: bool,
    pub all_5_files_present: bool,
    pub all_modes_0o600: bool,
    pub all_files_parse_as_json: bool,
    pub total_evidence_bytes: u64,
}
    </signature>
  </signatures>
  <constraints>
    - SEC-MEJEPA-07: every emitted FSV file MUST be mode 0o600 (verified at write-time AND re-verified at assert_fsv_directory_complete)
    - 5 files exactly (FSV_FILES.len() == 5); the `_master_index.json` is informational and NOT one of the 5 ship-gate files
    - Pretty-JSON format (indent 2; trailing newline) for human review; serde_json::to_string_pretty
    - Binary mock data PROHIBITED; example invokes run_heal_drill which uses synthetic-but-real fixtures (REQ-HEAL-16)
    - Each subsequent run OVERWRITES (not appends) the 5 files — the FSV directory always reflects the LATEST drill, not history
    - No `unwrap_or(...)` fallbacks; missing/invalid file → Err with full context
    - File &lt; 500 lines (each: src/heal/fsv.rs ~250 lines, examples/phase5_self_healing_fsv.rs ~250 lines)
    - SCREAMING_SNAKE error code: MEJEPA_HEAL_FSV_INCOMPLETE (introduce in TASK-HEAL-004 if not present, or subsume under existing MEJEPA_HEAL_* taxonomy with a new variant)
    - tracing::info! before each FSV file write; tracing::error! before any FsvIncomplete return
    - Exit code 0 iff `ship_gate_pass == true`; exit 1 otherwise (single-bit ship-gate output)
  </constraints>
  <verification>
    - `cargo test -p context-graph-mejepa heal::fsv` shows ≥ 14 passing tests
    - `cargo run --release -p context-graph-mejepa --example phase5_self_healing_fsv -- --inject-drift hard` exits 0
    - 5 files at /tmp/contextgraph-mejepa-self-healing-fsv/: drift-drill-evidence.json + abc-promotion-evidence.json + lora-refresh-evidence.json + witness-integrity-evidence.json + ewc-fisher-evidence.json
    - All 5 files mode 0o600 (verified via stat())
    - All 5 files parse as valid JSON
    - _master_index.json present (informational; mode 0o600)
    - ship_gate_pass=true on hard drift → exit 0
    - Force one file mode 0o644 → assert_fsv_directory_complete returns FsvIncomplete; exit 1
    - Re-running OVERWRITES (not appends); file count stays at 5
  </verification>
</definition_of_done>

<pseudo_code>
pub fn write_fsv_evidence_canonical<T: Serialize>(file_name: &str, payload: &T, root: Option<&Path>) -> Result<PathBuf, HealError> {
    let root = root.map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from(FSV_ROOT));
    fs::create_dir_all(&root).map_err(|e| HealError::FsvIncomplete { missing: vec![root.display().to_string()], wrong_mode: vec![], parse_failures: vec![format!("create_dir_all: {}", e)] })?;
    let path = root.join(file_name);
    let json = serde_json::to_string_pretty(payload).map_err(|e| HealError::FsvIncomplete { missing: vec![], wrong_mode: vec![], parse_failures: vec![format!("serialize {}: {}", file_name, e)] })?;
    fs::write(&path, json.as_bytes() + b"\n")?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    tracing::info!(path = %path.display(), "FSV evidence written mode 0o600");
    Ok(path)
}

pub fn assert_fsv_directory_complete(root: Option<&Path>) -> Result<(), HealError> {
    let root = root.map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from(FSV_ROOT));
    let mut missing = vec![];
    let mut wrong_mode = vec![];
    let mut parse_failures = vec![];
    for f in FSV_FILES {
        let p = root.join(f);
        if !p.exists() { missing.push(f.to_string()); continue; }
        let meta = fs::metadata(&p)?;
        let mode = meta.permissions().mode() & 0o777;
        if mode != 0o600 { wrong_mode.push((f.to_string(), mode)); }
        let bytes = fs::read(&p)?;
        if let Err(e) = serde_json::from_slice::<serde_json::Value>(&bytes) {
            parse_failures.push(format!("{}: {}", f, e));
        }
    }
    if !missing.is_empty() || !wrong_mode.is_empty() || !parse_failures.is_empty() {
        tracing::error!(error_code = "MEJEPA_HEAL_FSV_INCOMPLETE", ?missing, ?wrong_mode, ?parse_failures);
        return Err(HealError::FsvIncomplete { missing, wrong_mode, parse_failures });
    }
    Ok(())
}

// examples/phase5_self_healing_fsv.rs main:
fn main() -> Result<(), HealError> {
    let args = FsvHarnessArgs::parse();
    let root = args.output_root.unwrap_or_else(|| PathBuf::from(FSV_ROOT));
    fs::create_dir_all(&root)?;
    let drill_summary = mejepa_heal_drill::run_heal_drill(HealDrillArgs { inject_drift: args.inject_drift, output_fsv: root.clone(), max_observations: 5000, seed: args.seed, rtx_5090_budget_min: 60 })?;
    assert_fsv_directory_complete(Some(&root))?;
    append_fsv_master_index(Some(&root))?;
    let dir_summary = fsv_directory_summary(Some(&root))?;
    let report = FsvHarnessReport {
        drill_summary, fsv_directory_summary: dir_summary.clone(),
        ship_gate_pass: dir_summary.all_complete && dir_summary.file_modes.iter().all(|m| *m == 0o600),
        all_5_files_present: dir_summary.file_paths_present.len() == 5,
        all_modes_0o600: dir_summary.file_modes.iter().all(|m| *m == 0o600),
        all_files_parse_as_json: true, // assert_fsv_directory_complete guards
        total_evidence_bytes: dir_summary.total_bytes,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    if !report.ship_gate_pass { std::process::exit(1); }
    Ok(())
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/heal/fsv.rs">FSV_ROOT, FSV_FILES, write_fsv_evidence_canonical, assert_fsv_directory_complete, fsv_directory_summary, append_fsv_master_index, DriftDrillEvidence, EwcFisherEvidence, FsvDirectorySummary, FsvHarnessReport + tests</file>
  <file path="crates/context-graph-mejepa/examples/phase5_self_healing_fsv.rs">main + FsvHarnessArgs + drives mejepa_heal_drill::run_heal_drill + emits master FsvHarnessReport</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/Cargo.toml">add `[[example]] name = "phase5_self_healing_fsv"`; add `tempfile = { workspace = true }` + `clap = { workspace = true, features = ["derive"] }` to dev-dependencies if missing</file>
  <file path="crates/context-graph-mejepa/src/heal/mod.rs">add `pub mod fsv;`</file>
  <file path="crates/context-graph-mejepa/src/heal/errors.rs">add HealError::FsvIncomplete { missing: Vec&lt;String&gt;, wrong_mode: Vec&lt;(String, u32)&gt;, parse_failures: Vec&lt;String&gt; } variant + .code() returns "MEJEPA_HEAL_FSV_INCOMPLETE"</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo test -p context-graph-mejepa heal::fsv shows ≥ 14 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>cargo run --release -p context-graph-mejepa --example phase5_self_healing_fsv -- --inject-drift hard exits 0</criterion>
  <criterion>5 FSV files at /tmp/contextgraph-mejepa-self-healing-fsv/ (drift-drill-evidence.json + abc-promotion-evidence.json + lora-refresh-evidence.json + witness-integrity-evidence.json + ewc-fisher-evidence.json)</criterion>
  <criterion>All 5 files mode 0o600 (verified via stat())</criterion>
  <criterion>All 5 files parse as valid JSON</criterion>
  <criterion>_master_index.json informational file present (mode 0o600)</criterion>
  <criterion>Force one file to mode 0o644 → assert_fsv_directory_complete returns FsvIncomplete; example exits 1</criterion>
  <criterion>Re-running overwrites (not appends); file count stays at 5</criterion>
  <criterion>Soft / Hard / Catastrophic drift bands all produce 5-file FSV output (varying mode_winner / status_change but all 5 files always emitted)</criterion>
  <criterion>FsvHarnessReport printed to stdout with ship_gate_pass + 4 boolean gate flags</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa heal::fsv</command>
  <command>cargo run --release -p context-graph-mejepa --example phase5_self_healing_fsv -- --inject-drift hard --seed 42</command>
  <command>cargo run --release -p context-graph-mejepa --example phase5_self_healing_fsv -- --inject-drift catastrophic --seed 43</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
