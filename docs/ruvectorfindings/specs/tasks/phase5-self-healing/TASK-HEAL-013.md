# TASK-HEAL-013 — ChainIntegrityChecker::verify (chain.len() % 73 + replay; halt + restore on break)

```xml
<task_spec id="TASK-HEAL-013" version="1.0">

<metadata>
  <title>Witness-chain integrity verifier in `crates/context-graph-mejepa/src/heal/integrity.rs` (delegates to shipped context-graph-witness)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>13</sequence>
  <implements>
    <item ref="REQ-HEAL-07">Chain integrity check: chain.len() % 73 == 0 + prev_hash replay; halt + restore on break</item>
    <item ref="ERR-HEAL-03">MEJEPA_OBSERVE_INTEGRITY_VIOLATION (status_change=paused; restore last good promotion's weights_sha + calibration version; SMAP blocker severity:critical)</item>
    <item ref="EC-HEAL-08">Legacy first-entry prev_hash mismatch → reconcile via shipped witness_chain_legacy; if not legacy → BrokenAt(0)</item>
  </implements>
  <depends_on>
    <task_ref>TASK-HEAL-004</task_ref>
    <task_ref>TASK-HEAL-005</task_ref>
    <task_ref>TASK-HEAL-008</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (src/heal/integrity.rs); 1 modified (src/heal/mod.rs)</estimated_files>
</metadata>

<context>
Logic 8 of 10. Implements the witness-chain integrity verifier that is fired every 10,000
observations OR on drift detection. Chain integrity is the bedrock of model-promotion auditability:
if the chain breaks, every downstream guarantee (rollback, holdout-evaluation provenance, signed
ModelPromote entries) collapses. This task is `ready` (not blocked) because the heavy lifting is
already shipped in `crates/context-graph-witness` (verify_chain_integrity, read_witness_chain,
witness_chain_legacy::reconcile, witness_chain_repair, ed25519 segment signing). This task is the
THIN wrapper that wires the shipped machinery into the heal cycle's break-recovery path:
status_change=paused, restore from last verified checkpoint, surface SMAP blocker severity:critical.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (`<failure_taxonomy>` MEJEPA_OBSERVE_INTEGRITY_VIOLATION; SEC-MEJEPA-03 ed25519; CLAUDE.md §6 SMAP frontmatter on every memory/blockers/ entry)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-SELF-HEALING.md (REQ-HEAL-07; AC-HEAL-08-01..03; EC-HEAL-08; ERR-HEAL-03)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-SELF-HEALING.md (component_contracts ChainIntegrityChecker::verify 6-step body; data_models ChainIntegrityChecker, IntegrityViolationKind)</file>
  <file purpose="prior_task">TASK-HEAL-004 (HealError, IntegrityViolationKind, ChainIntegrityChecker struct)</file>
  <file purpose="prior_task">TASK-HEAL-005 (RocksDB CFs)</file>
  <file purpose="prior_task">TASK-HEAL-008 (SelfHealingPipeline orchestrator that calls verify() every 10,000 obs)</file>
  <file purpose="shipped_witness_chain">crates/context-graph-witness/src/witness_chain_format.rs (73-byte SHAKE-256 entry layout)</file>
  <file purpose="shipped_witness_chain">crates/context-graph-witness/src/witness_chain_legacy.rs (reconcile shim for pre-rev-2 chains)</file>
  <file purpose="shipped_witness_chain">crates/context-graph-witness/src/witness_chain_repair.rs (operator-approved repair tool)</file>
  <file purpose="shipped_witness_chain">crates/context-graph-witness/src/cert.rs (verify_chain_integrity + read_witness_chain shipped APIs)</file>
</input_context_files>

<prerequisites>
  <check>TASK-HEAL-004 ChainIntegrityChecker struct + IntegrityViolationKind enum complete</check>
  <check>TASK-HEAL-005 CF_MEJEPA_HEAL_REPORTS available (for last-good-promotion lookup)</check>
  <check>TASK-HEAL-008 SelfHealingPipeline exposes `pipeline.witness_chain` (Arc&lt;WitnessChainAppender&gt;) + `pipeline.status` (Arc&lt;Mutex&lt;HealStatus&gt;&gt;)</check>
  <check>Shipped: `context_graph_witness::verify_chain_integrity(chain_path: &Path) -> Result<ChainReport, WitnessError>`</check>
  <check>Shipped: `context_graph_witness::read_witness_chain(chain_path: &Path) -> Result<Vec<WitnessEntry>, WitnessError>`</check>
  <check>Shipped: `context_graph_witness::witness_chain_legacy::reconcile(entries: &[WitnessEntry]) -> Result<LegacyReconcileReport, WitnessError>`</check>
  <check>Shipped: `context_graph_witness::witness_chain_repair::propose_repair(...)` (operator-approved; not auto-invoked)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/heal/integrity.rs`:
      - `pub fn verify(checker: &mut ChainIntegrityChecker, witness_chain: &WitnessChainAppender, storage: &RocksDbStore, status: &Arc<Mutex<HealStatus>>) -> Result<ChainIntegrityReport, HealError>` per component_contracts (6-step body):
        1. Read durable chain bytes via `context_graph_witness::read_witness_chain(&checker.chain_path)?`
        2. Assert `chain.bytes.len() % 73 == 0`; else: stamp `status.status_change = StatusChange::Paused`; surface SMAP blocker severity:critical (write `memory/blockers/agent-{N}-mejepa-heal--chain-wrong-size.md` with frontmatter); return `Err(HealError::IntegrityViolation { kind: IntegrityViolationKind::WrongSize { actual: chain.bytes.len() as u64, expected_modulo: 73 } })`
        3. Iterate entries; for each entry compute `prev_hash = shake256_256(prev_entry_bytes)`; assert `entry.prev_hash == prev_hash`; on first mismatch at index `i`:
           - If `i == 0` (legacy first-entry case per EC-HEAL-08): try `witness_chain_legacy::reconcile(&entries)`; if `LegacyReconcileReport::ConfirmedLegacy`: invoke `witness_chain_repair::propose_repair(...)` to produce a repair plan, persist to memory/journal/, write a memory/blockers/ severity:high entry asking operator to approve; on approved repair, replay continues; on denied repair, return `Err(IntegrityViolation { kind: BrokenAt(0, expected_prev_hash, actual_prev_hash) })`
           - Else (i > 0 OR not legacy at i=0): stamp `status.status_change = StatusChange::Paused`; set `checker.rollback_target = scan_last_good_modelpromote_weights_sha(&storage, witness_chain, i)?`; surface SMAP blocker severity:critical at `memory/blockers/agent-{N}-mejepa-heal--chain-broken-at-{i}.md`; return `Err(IntegrityViolation { kind: BrokenAt { offset: i, expected_prev_hash, actual_prev_hash } })`
        4. On success: persist `ChainIntegrityReport { entry_count, replay_duration_ms, last_verified_offset }` via `write_text_checked` to `memory/journal/agent-{N}-mejepa-heal--chain-verified-{timestamp}.md` (with SMAP frontmatter)
        5. Update `checker.last_verified_offset = entries.len() as u64`
        6. Stamp `status.last_integrity_check_at = now_secs()`
        7. Return `Ok(ChainIntegrityReport)`
      - Helper: `pub fn scan_last_good_modelpromote_weights_sha(storage: &RocksDbStore, witness_chain: &WitnessChainAppender, broken_offset: usize) -> Result<[u8; 32], HealError>` — scans witness chain entries BEFORE `broken_offset` in reverse for the latest `op_type=ModelPromote`; returns its `weights_sha`; returns Err(IntegrityViolation { kind: NoGoodCheckpoint }) if none exist (catastrophic — operator escalation)
      - Helper: `pub fn write_smap_blocker_critical(reason: &str, kind: &IntegrityViolationKind, rollback_target: Option<&[u8; 32]>) -> Result<(), HealError>` — writes `./memory/blockers/agent-{N}-mejepa-heal--chain-broken-at-{i}.md` with SMAP frontmatter (namespace, agent_id, timestamp, status, tags) per CLAUDE.md §6
      - Helper: `pub fn write_smap_journal_chain_verified(report: &ChainIntegrityReport) -> Result<(), HealError>` — writes the success-path SMAP journal entry
    - Tests:
      - `verify_passes_on_clean_chain_of_100_entries` (real chain with valid prev_hashes)
      - `verify_returns_wrong_size_on_chain_not_multiple_of_73`
      - `verify_returns_broken_at_offset_i_on_corrupted_prev_hash` (inject one corrupted byte at known offset)
      - `verify_stamps_status_change_paused_on_break`
      - `verify_writes_smap_blocker_severity_critical_on_break` (assert frontmatter + namespace=blockers)
      - `verify_sets_rollback_target_to_last_good_modelpromote_weights_sha`
      - `verify_returns_legacy_first_entry_repaired_on_ec_heal_08` (legacy chain reconciles cleanly)
      - `verify_returns_broken_at_zero_when_first_entry_not_legacy_format` (non-legacy mismatch at 0)
      - `verify_persists_journal_entry_on_success_with_smap_frontmatter`
      - `verify_updates_last_verified_offset_on_success`
      - `verify_updates_status_last_integrity_check_at_on_success`
      - `scan_last_good_modelpromote_returns_latest_pre_break_weights_sha`
      - `scan_last_good_modelpromote_returns_no_good_checkpoint_when_none_exist`
      - `verify_real_chain_under_100k_entries_completes_under_500ms` (performance budget; soft-asserted via tracing-recorded duration)
      - End-to-end: write 1,000-entry chain, corrupt at offset 500, verify returns BrokenAt(500)
    - FSV evidence emission for TASK-HEAL-019: `IntegrityEvidence { entry_count, first_entry_sha, last_entry_sha, replay_duration_ms, status_change_after, rollback_target_sha, broken_at_offset, blocker_path }`
  </in_scope>
  <out_of_scope>
    - The witness_chain_format.rs / witness_chain_legacy.rs / witness_chain_repair.rs / cert.rs implementations themselves (all SHIPPED in crates/context-graph-witness)
    - The actual rollback execution (TASK-HEAL-010 rollback_to is the surface; this task only IDENTIFIES the rollback target)
    - Operator UI for repair-approval (out of MEJEPA scope; surface via SMAP blocker)
    - The MCP tool surface (TASK-HEAL-016 / TASK-HEAL-017)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/heal/integrity.rs">
use std::sync::{Arc, Mutex};
use std::path::Path;
use crate::heal::{errors::HealError, status::{HealStatus, StatusChange},
                  data_models::{ChainIntegrityChecker, IntegrityViolationKind, ChainIntegrityReport}};
use context_graph_witness::{verify_chain_integrity, read_witness_chain,
                            witness_chain_legacy, witness_chain_repair,
                            WitnessChainAppender, WitnessEntry};

pub fn verify(
    checker: &amp;mut ChainIntegrityChecker,
    witness_chain: &amp;WitnessChainAppender,
    storage: &amp;RocksDbStore,
    status: &amp;Arc&lt;Mutex&lt;HealStatus&gt;&gt;,
) -&gt; Result&lt;ChainIntegrityReport, HealError&gt;;

pub fn scan_last_good_modelpromote_weights_sha(
    storage: &amp;RocksDbStore,
    witness_chain: &amp;WitnessChainAppender,
    broken_offset: usize,
) -&gt; Result&lt;[u8; 32], HealError&gt;;

pub fn write_smap_blocker_critical(
    reason: &amp;str,
    kind: &amp;IntegrityViolationKind,
    rollback_target: Option&lt;&amp;[u8; 32]&gt;,
) -&gt; Result&lt;(), HealError&gt;;

pub fn write_smap_journal_chain_verified(
    report: &amp;ChainIntegrityReport,
) -&gt; Result&lt;(), HealError&gt;;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IntegrityEvidence {
    pub entry_count: u64,
    pub first_entry_sha: [u8; 32],
    pub last_entry_sha: [u8; 32],
    pub replay_duration_ms: u64,
    pub status_change_after: StatusChange,
    pub rollback_target_sha: Option&lt;[u8; 32]&gt;,
    pub broken_at_offset: Option&lt;usize&gt;,
    pub blocker_path: Option&lt;String&gt;,
}
    </signature>
  </signatures>
  <constraints>
    - DELEGATE to shipped `context-graph-witness` for read_witness_chain + verify_chain_integrity + witness_chain_legacy::reconcile + witness_chain_repair (NEVER reinvent these — already shipped per `_index.md` Reuse note)
    - On break: status_change=Paused MUST be stamped BEFORE Err is returned (so in-flight RealityPredictions inherit the paused state)
    - SMAP blocker MUST have frontmatter per CLAUDE.md §6: namespace=blockers, agent_id, timestamp (ISO 8601 UTC), status=active, tags=mejepa,heal,integrity
    - SMAP journal MUST have frontmatter per CLAUDE.md §6: namespace=journal, agent_id, timestamp, status=resolved, tags=mejepa,heal,integrity
    - Legacy reconciliation (EC-HEAL-08) is OPERATOR-APPROVED ONLY: this task PROPOSES a repair via witness_chain_repair but does NOT auto-execute (operator approval is a manual gate)
    - rollback_target MUST be a real ModelPromote weights_sha BEFORE the broken_offset (never speculative)
    - File &lt; 500 lines
    - SCREAMING_SNAKE error code: MEJEPA_OBSERVE_INTEGRITY_VIOLATION
    - tracing::error! before every Err return; tracing::info! on success
    - No mock data in tests; real chain bytes (synthetic but valid SHAKE-256 prev_hash)
  </constraints>
  <verification>
    - `cargo test -p context-graph-mejepa heal::integrity` shows ≥ 15 passing tests
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - Clean 100-entry chain → Ok(ChainIntegrityReport) + journal entry written
    - chain.len() % 73 != 0 → IntegrityViolation { kind: WrongSize } + status_change=Paused + blocker written
    - Corrupted entry at offset 500 → IntegrityViolation { kind: BrokenAt(500) } + rollback_target set
    - Legacy first-entry mismatch → reconcile path engaged; repair PROPOSED (not auto-applied)
    - SMAP frontmatter validated on every blocker + journal entry
    - 100,000-entry chain replay completes &lt; 500ms (soft-asserted via tracing duration)
  </verification>
</definition_of_done>

<pseudo_code>
pub fn verify(checker, witness_chain, storage, status) -> Result<ChainIntegrityReport, HealError> {
    let started_at = Instant::now();
    let entries = read_witness_chain(&checker.chain_path)
        .map_err(|e| HealError::IntegrityViolation { kind: IntegrityViolationKind::WrongSize { actual: 0, expected_modulo: 73 } })?;
    let chain_bytes_len: u64 = entries.iter().map(|e| e.byte_len() as u64).sum();
    if chain_bytes_len % 73 != 0 {
        status.lock().unwrap().status_change = StatusChange::Paused;
        let kind = IntegrityViolationKind::WrongSize { actual: chain_bytes_len, expected_modulo: 73 };
        write_smap_blocker_critical("chain wrong-size", &kind, None)?;
        tracing::error!(error_code = "MEJEPA_OBSERVE_INTEGRITY_VIOLATION", actual = chain_bytes_len);
        return Err(HealError::IntegrityViolation { kind });
    }
    for (i, entry) in entries.iter().enumerate() {
        let expected_prev = if i == 0 { [0u8; 32] } else { shake256_256(&entries[i-1].as_bytes()) };
        if entry.prev_hash != expected_prev {
            if i == 0 {
                match witness_chain_legacy::reconcile(&entries) {
                    Ok(LegacyReconcileReport::ConfirmedLegacy) => {
                        let plan = witness_chain_repair::propose_repair(...)?;
                        write_smap_blocker_high("legacy first-entry; awaiting operator approval", ...)?;
                        // Continue (operator approval is async; verifier returns repair-pending)
                    }
                    _ => { /* fall-through to BrokenAt(0) */ }
                }
            }
            status.lock().unwrap().status_change = StatusChange::Paused;
            let rollback_target = scan_last_good_modelpromote_weights_sha(storage, witness_chain, i)?;
            checker.rollback_target = Some(rollback_target);
            let kind = IntegrityViolationKind::BrokenAt { offset: i, expected_prev_hash: expected_prev, actual_prev_hash: entry.prev_hash };
            write_smap_blocker_critical("chain broken", &kind, Some(&rollback_target))?;
            tracing::error!(error_code = "MEJEPA_OBSERVE_INTEGRITY_VIOLATION", offset = i);
            return Err(HealError::IntegrityViolation { kind });
        }
    }
    let replay_duration_ms = started_at.elapsed().as_millis() as u64;
    let report = ChainIntegrityReport {
        entry_count: entries.len() as u64,
        first_entry_sha: sha256(&entries[0].as_bytes()),
        last_entry_sha: sha256(&entries[entries.len()-1].as_bytes()),
        replay_duration_ms,
        last_verified_offset: entries.len() as u64,
    };
    write_smap_journal_chain_verified(&report)?;
    checker.last_verified_offset = entries.len() as u64;
    status.lock().unwrap().last_integrity_check_at = now_secs();
    Ok(report)
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/heal/integrity.rs">verify + scan_last_good_modelpromote_weights_sha + write_smap_blocker_critical + write_smap_journal_chain_verified + IntegrityEvidence + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/heal/mod.rs">add `pub mod integrity;`; re-export verify + ChainIntegrityReport</file>
  <file path="crates/context-graph-mejepa/Cargo.toml">add `context-graph-witness = { workspace = true }` if not already present</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo test -p context-graph-mejepa heal::integrity shows ≥ 15 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>Clean chain returns Ok(ChainIntegrityReport)</criterion>
  <criterion>Wrong-size chain → IntegrityViolation { kind: WrongSize } + status_change=Paused</criterion>
  <criterion>Corrupted prev_hash → IntegrityViolation { kind: BrokenAt(i) } + rollback_target set</criterion>
  <criterion>Legacy first-entry → reconcile path engaged + repair PROPOSED (not auto-applied)</criterion>
  <criterion>SMAP blocker severity:critical written on break with frontmatter (namespace, agent_id, timestamp, status, tags)</criterion>
  <criterion>SMAP journal entry written on success with frontmatter</criterion>
  <criterion>100,000-entry chain replay &lt; 500ms (soft via tracing)</criterion>
  <criterion>scan_last_good_modelpromote_weights_sha returns latest pre-break ModelPromote</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa heal::integrity</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
