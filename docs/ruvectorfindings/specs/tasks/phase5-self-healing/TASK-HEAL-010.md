# TASK-HEAL-010 — AbcPromoter::rollback_to (operator-only; ModelRollback witness entry; CF_MEJEPA_WEIGHT_BLOBS lookup)

```xml
<task_spec id="TASK-HEAL-010" version="1.0">

<metadata>
  <title>Operator-only rollback method body in `crates/context-graph-mejepa/src/heal/promote.rs` (extends TASK-HEAL-009)</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>10</sequence>
  <implements>
    <item ref="REQ-HEAL-18">mcp__cgreality__mejepa_rollback_to operator-only rollback to prior promotion's weights_sha</item>
    <item ref="BR-HEAL-04">Witness-chain entry on every weight promotion (rollback is a promotion-class write)</item>
    <item ref="EC-HEAL-14">Operator issues rollback_to for offset whose weights_sha was already GC'd by Phase 6</item>
    <item ref="ERR-HEAL-08">MEJEPA_HEAL_PROMOTION_DEADLOCK (rollback respects same lock as A/B/C)</item>
    <item ref="ERR-HEAL-11">MEJEPA_HEAL_ROLLBACK_TARGET_GONE</item>
  </implements>
  <depends_on>
    <task_ref>TASK-HEAL-003</task_ref>
    <task_ref>TASK-HEAL-004</task_ref>
    <task_ref>TASK-HEAL-005</task_ref>
    <task_ref>TASK-HEAL-009</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 modified (src/heal/promote.rs); 1 created (src/heal/rollback.rs if size cap requires split)</estimated_files>
</metadata>

<context>
Logic 5 of 10. Implements the operator-only rollback path that reverses an earlier promotion by
re-pointing the active-weights pointer to a prior promotion's weights_sha, recorded as a
ModelRollback witness entry chained from the latest entry. The target weights_sha MUST still be
in CF_MEJEPA_WEIGHT_BLOBS (within one promotion-cycle retention per Phase 6 hygiene); if GC'd
already, returns RollbackTargetGone with the GC timestamp. Reuses the same promotion lock as
TASK-HEAL-009 to guarantee no concurrent A/B/C is in flight.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (`<failure_taxonomy>` MEJEPA_HEAL_ROLLBACK_TARGET_GONE; SEC-MEJEPA-03 ed25519 signing; `<security_requirements>` operator-only)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-SELF-HEALING.md (REQ-HEAL-18; AC-HEAL-12-01..03; EC-HEAL-14)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-SELF-HEALING.md (component_contracts AbcPromoter::rollback_to 8-step body)</file>
  <file purpose="prior_task">TASK-HEAL-003 (AbcPromoter struct + ModelHandle)</file>
  <file purpose="prior_task">TASK-HEAL-004 (HealReport, HealError, PromotionLockState)</file>
  <file purpose="prior_task">TASK-HEAL-005 (CF_MEJEPA_WEIGHT_BLOBS, CF_MEJEPA_ACTIVE_POINTERS, CF_MEJEPA_HEAL_REPORTS)</file>
  <file purpose="prior_task">TASK-HEAL-009 (try_acquire_promotion_lock + release_promotion_lock + sha256 helpers)</file>
  <file purpose="cross_domain_witness">crates/context-graph-witness/src/witness_chain.rs (WitnessChainAppender + WitnessOp::ModelRollback variant + ed25519 per SEC-MEJEPA-03; shipped)</file>
  <file purpose="cross_domain_hygiene">docs/ruvectorfindings/specs/functional/SPEC-HYGIENE.md (CF_MEJEPA_WEIGHT_BLOBS retention contract: active + 1 prior promotion-cycle; older GC'd)</file>
</input_context_files>

<prerequisites>
  <check>TASK-HEAL-003..005 + TASK-HEAL-009 complete</check>
  <check>WitnessChainAppender supports `WitnessOp::ModelRollback { weights_sha, source_offset }` (op_type registry exposes the variant)</check>
  <check>HealReport::variant supports trigger_reason=OperatorTriggered</check>
  <check>RocksDbStore exposes typed `get(CF_MEJEPA_WEIGHT_BLOBS, theta_sha) -> Option<Vec<u8>>`</check>
</prerequisites>

<scope>
  <in_scope>
    - Implement on `AbcPromoter`:
      - `pub fn rollback_to(&mut self, target_witness_chain_offset: u64, predictor: Arc<RwLock<Predictor>>, storage: Arc<RocksDbStore>, witness_chain: Arc<WitnessChainAppender>, lock: Arc<Mutex<PromotionLockState>>) -> Result<HealReport, HealError>` per component_contracts (8-step body):
        1. `try_acquire_promotion_lock(lock, TriggerReason::OperatorTriggered)`; if held, return Err(PromotionDeadlock { holder })
        2. Read entry at `target_witness_chain_offset` via `witness_chain.read_entry(target_witness_chain_offset)?`; assert `entry.op_type == WitnessOp::ModelPromote`; else return Err(IntegrityViolation { kind: WrongTargetOpType { found, expected: ModelPromote } })
        3. Extract `weights_sha` from entry payload (32 bytes; assert length else return Err(IntegrityViolation { kind: BrokenAt(target_witness_chain_offset as usize) }))
        4. Look up `weights_sha` in CF_MEJEPA_WEIGHT_BLOBS via `storage.get(CF_MEJEPA_WEIGHT_BLOBS, &weights_sha)?`; if `None`, return Err(RollbackTargetGone { theta_sha: weights_sha, gced_at: scan_heal_reports_for_gc_timestamp(&storage, &weights_sha)? })
        5. Append `ModelRollback` witness entry: `WitnessOp::ModelRollback { weights_sha, source_offset: target_witness_chain_offset }`; signed with ed25519 per SEC-MEJEPA-03; capture `new_witness_chain_offset`
        6. Update `CF_MEJEPA_ACTIVE_POINTERS["active_weights"]` atomically via write_text_checked (staging + fsync + atomic-rename)
        7. Hot-swap predictor θ via `Arc<RwLock<Predictor>>` write lock: `predictor.write().unwrap().load_weights(&blob)?`
        8. Release promotion_lock; return `HealReport` with `mode_winner=A` (rolled-back theta is the new active), `weights_sha_winner=weights_sha`, `witness_chain_offset=new_witness_chain_offset`, `trigger_reason=OperatorTriggered`, `status_change=StatusChange::Active`
    - Implement helpers:
      - `fn scan_heal_reports_for_gc_timestamp(storage: &RocksDbStore, target_sha: &[u8; 32]) -> Result<i64, HealError>` — scans CF_MEJEPA_HEAL_REPORTS in reverse for the latest report referencing `target_sha`; returns the report's timestamp+retention-cycle as the inferred GC moment; defaults to 0 if not found (informational only — caller still treats target as gone)
      - `fn read_modelpromote_entry(witness_chain: &WitnessChainAppender, offset: u64) -> Result<ModelPromoteEntry, HealError>` — wraps `witness_chain.read_entry` + payload-decoding + op_type assertion
    - HealReport extension (ONLY if not already in TASK-HEAL-004): `trigger_reason: TriggerReason::OperatorTriggered` variant. If absent in TASK-HEAL-004, add the variant without breaking existing HealReport bincode layout (append-only enum variants per BR rule).
    - Tests:
      - `rollback_to_succeeds_when_target_offset_is_modelpromote_with_blob_present`
      - `rollback_to_returns_rollback_target_gone_when_blob_gced` (EC-HEAL-14)
      - `rollback_to_returns_integrity_violation_when_target_offset_not_modelpromote`
      - `rollback_to_returns_promotion_deadlock_when_lock_held` (concurrent A/B/C)
      - `rollback_to_appends_signed_modelrollback_witness_entry`
      - `rollback_to_witness_entry_chains_to_prior_entry` (prev_hash matches)
      - `rollback_to_active_pointer_atomic_after_witness_entry`
      - `rollback_to_predictor_hotswapped_with_target_weights`
      - `rollback_to_persists_healreport_with_operator_triggered`
      - `rollback_to_releases_lock_on_terminal_error` (RAII-style; verified via concurrent retry succeeds after Err)
      - `read_modelpromote_entry_rejects_modelrollback_op_type`
      - `scan_heal_reports_for_gc_timestamp_returns_latest_report_ts_when_present`
    - FSV evidence emission for TASK-HEAL-019: extend `AbcFsvEvidence` (TASK-HEAL-009) with optional `RollbackEvidence { target_witness_chain_offset, target_weights_sha, new_witness_chain_offset, hot_swap_latency_ms }` populated when `rollback_to` completes successfully.
  </in_scope>
  <out_of_scope>
    - retrain_and_promote (TASK-HEAL-009)
    - The MCP-tool surface itself (TASK-HEAL-017 wraps this method)
    - Phase 6 hygiene GC actually pruning blobs (cross-cutting; this task only READS the post-GC state)
    - WitnessChain ModelRollback schema (assumed shipped; see prerequisites)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/heal/promote.rs">
impl AbcPromoter {
    pub fn rollback_to(
        &amp;mut self,
        target_witness_chain_offset: u64,
        predictor: Arc&lt;RwLock&lt;Predictor&gt;&gt;,
        storage: Arc&lt;RocksDbStore&gt;,
        witness_chain: Arc&lt;WitnessChainAppender&gt;,
        lock: Arc&lt;Mutex&lt;PromotionLockState&gt;&gt;,
    ) -&gt; Result&lt;HealReport, HealError&gt;;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelPromoteEntry {
    pub weights_sha: [u8; 32],
    pub mode_winner: ModeWinner,
    pub evaluation_summary_sha: [u8; 32],
    pub timestamp_seconds: i64,
    pub op_type: WitnessOp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RollbackEvidence {
    pub target_witness_chain_offset: u64,
    pub target_weights_sha: [u8; 32],
    pub new_witness_chain_offset: u64,
    pub hot_swap_latency_ms: u64,
}
    </signature>
  </signatures>
  <constraints>
    - rollback_to MUST be called only by the MCP tool surface (TASK-HEAL-017) — no internal scheduler triggers it; enforced via doc-comment + the MCP tool's caller_role=Operator gate per `<security_requirements>`
    - Promotion lock is the SAME lock used by retrain_and_promote (TASK-HEAL-009) — concurrent A/B/C blocks rollback and vice versa
    - Witness entry MUST be appended BEFORE the active-pointer update (atomic ordering; on crash between the two, recovery routine from TASK-HEAL-009 replays the latest top-of-chain op_type)
    - RollbackTargetGone is INFORMATIONAL not destructive — the active weights are unchanged on this error
    - File &lt; 500 lines (split into rollback.rs if needed; promote.rs already houses retrain_and_promote)
    - tracing::error! before every Err return per failure_logging note
    - SCREAMING_SNAKE error codes: MEJEPA_HEAL_ROLLBACK_TARGET_GONE, MEJEPA_HEAL_PROMOTION_DEADLOCK, MEJEPA_OBSERVE_INTEGRITY_VIOLATION
    - HealReport.status_change=StatusChange::Active after successful rollback (the operator-driven path is fast: no retraining cycle)
    - No mock data in tests; use real RocksDB tempdir + real ed25519 keypair from a tempfile keyring per CLAUDE.md §11
  </constraints>
  <verification>
    - `cargo test -p context-graph-mejepa heal::promote::rollback` shows ≥ 12 new tests
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - RollbackTargetGone returned with `theta_sha` payload populated
    - PromotionDeadlock returned on concurrent attempt
    - WitnessChain entry appended with op_type=ModelRollback + valid ed25519 signature + prev_hash chains to last entry
    - Active-pointer updated atomically AFTER witness entry
    - On crash between witness entry and active-pointer update, recovery routine from TASK-HEAL-009 replays the latest ModelRollback (EC-HEAL-13 generalization)
  </verification>
</definition_of_done>

<pseudo_code>
pub fn rollback_to(&mut self, target_witness_chain_offset, predictor, storage, witness_chain, lock) -> Result<HealReport, HealError> {
    let started_at = SystemTime::now();
    try_acquire_promotion_lock(&lock, TriggerReason::OperatorTriggered)?;
    let result = (|| -> Result<HealReport, HealError> {
        // Step 2: read entry, assert op_type
        let entry = read_modelpromote_entry(&witness_chain, target_witness_chain_offset)?;
        let target_sha = entry.weights_sha;
        // Step 4: look up blob (RollbackTargetGone if missing)
        let blob = storage.get(CF_MEJEPA_WEIGHT_BLOBS, &target_sha)?
            .ok_or_else(|| {
                let gced_at = scan_heal_reports_for_gc_timestamp(&storage, &target_sha).unwrap_or(0);
                tracing::error!(error_code = "MEJEPA_HEAL_ROLLBACK_TARGET_GONE", target_sha = ?target_sha, gced_at);
                HealError::RollbackTargetGone { theta_sha: target_sha, gced_at }
            })?;
        // Step 5: append ModelRollback (signed)
        let new_offset = witness_chain.append_signed(WitnessOp::ModelRollback {
            weights_sha: target_sha,
            source_offset: target_witness_chain_offset,
        })?;
        // Step 6: atomic active-pointer update
        let active_value = ActivePointerValue { theta_sha_or_version: target_sha.to_vec(), frozen_at: now_secs() };
        storage.put_atomic(CF_MEJEPA_ACTIVE_POINTERS, b"active_weights", &bincode::serialize(&active_value)?)?;
        // Step 7: hot-swap predictor
        let hot_swap_start = std::time::Instant::now();
        predictor.write().unwrap().load_weights(&blob)?;
        let hot_swap_latency_ms = hot_swap_start.elapsed().as_millis() as u64;
        // Build HealReport with operator-triggered rollback (mode_winner=A; the rolled-back θ is the new active)
        let report = HealReport::try_new_rollback(target_sha, new_offset, started_at.elapsed().unwrap().as_secs(), TriggerReason::OperatorTriggered)?;
        storage.put(CF_MEJEPA_HEAL_REPORTS, &encode_heal_report_key(now_secs()), &bincode::serialize(&report)?)?;
        Ok(report)
    })();
    release_promotion_lock(&lock);
    result
}
</pseudo_code>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/heal/promote.rs">add rollback_to body + ModelPromoteEntry codec + RollbackEvidence + 12 tests</file>
  <file path="crates/context-graph-mejepa/src/heal/mod.rs">re-export rollback_to surface (operator-only doc-comment)</file>
</files_to_modify>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/heal/rollback.rs">scan_heal_reports_for_gc_timestamp + read_modelpromote_entry helpers (split if size cap requires)</file>
</files_to_create>

<validation_criteria>
  <criterion>cargo test -p context-graph-mejepa heal::promote::rollback shows ≥ 12 new passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>Missing target weights_sha → RollbackTargetGone with .code() == "MEJEPA_HEAL_ROLLBACK_TARGET_GONE"</criterion>
  <criterion>Concurrent A/B/C in flight → PromotionDeadlock with .code() == "MEJEPA_HEAL_PROMOTION_DEADLOCK"</criterion>
  <criterion>target_offset references non-ModelPromote entry → IntegrityViolation</criterion>
  <criterion>WitnessChain entry persisted with op_type=ModelRollback + ed25519 signature + chained prev_hash</criterion>
  <criterion>Active-pointer updated atomically AFTER witness entry</criterion>
  <criterion>Predictor hot-swapped with target weights (verified via parameters_flat() sha256 == target_sha)</criterion>
  <criterion>HealReport persisted with trigger_reason=OperatorTriggered + status_change=Active</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa heal::promote::rollback</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
