# TASK-HEAL-009 — AbcPromoter::retrain_and_promote (3-mode parallel; holdout gate; ModelPromote witness entry)

```xml
<task_spec id="TASK-HEAL-009" version="1.0">

<metadata>
  <title>A/B/C promotion method body in `crates/context-graph-mejepa/src/heal/promote.rs` (extending TASK-HEAL-003 struct shapes)</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>9</sequence>
  <implements>
    <item ref="REQ-HEAL-04">retrain_and_promote 3-mode + holdout gate</item>
    <item ref="REQ-HEAL-12">ModelPromote witness-chain entry (timestamp + op_type + weights_sha + mode_winner + evaluation_summary_sha)</item>
    <item ref="BR-HEAL-04">Witness-chain entry on every weight promotion</item>
    <item ref="BR-HEAL-07">No silent fallback when no mode passes 3 gates</item>
    <item ref="EC-HEAL-05">NaN mid-retrain releases lock; Mode A retained; severity:high blocker</item>
    <item ref="EC-HEAL-06">Grid-search w_A=1 result returns A_unchanged informational</item>
    <item ref="EC-HEAL-12">Concurrent drift mid-promotion: lock held; new drift journaled but no second retrain</item>
    <item ref="EC-HEAL-13">Crash between witness entry and active-pointer update; recovery via latest ModelPromote replay</item>
    <item ref="ERR-HEAL-04">MEJEPA_RETRAIN_HOLDOUT_REGRESSION</item>
    <item ref="ERR-HEAL-05">MEJEPA_RETRAIN_QUOTA_EXCEEDED</item>
    <item ref="ERR-HEAL-08">MEJEPA_HEAL_PROMOTION_DEADLOCK</item>
  </implements>
  <depends_on>
    <task_ref>TASK-HEAL-003</task_ref>
    <task_ref>TASK-HEAL-004</task_ref>
    <task_ref>TASK-HEAL-005</task_ref>
    <task_ref>TASK-HEAL-008</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>1 modified (src/heal/promote.rs); 1 created (src/heal/promote_holdout.rs if size cap requires split)</estimated_files>
</metadata>

<context>
Logic 4 of 10. Implements the canonical A/B/C three-mode parallel evaluation + 3-gate promotion
filter (coverage ≥ stated, oracle_agreement ≥ A's, OOD KL ≤ A's) + ModelPromote witness-chain entry
+ atomic active-weight pointer update. The Mode B branch invokes Phase 3
`train_from_scratch(snapshot)` against an immutable corpus snapshot — this is the wallclock long-pole
of the &lt;60min Phase 5 DoD. Mode C is a small grid-search (w_A ∈ {0.0, 0.1, ..., 1.0}, step 0.1) over
the holdout split. Promotion lock is exclusive and held for the entire call.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (`<failure_taxonomy>` MEJEPA_RETRAIN_*; SEC-MEJEPA-03 ed25519 signing; `<security_requirements>`)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-SELF-HEALING.md (REQ-HEAL-04, REQ-HEAL-12, BR-HEAL-04/07; AC-HEAL-03-01..05; EC-HEAL-05/06/12/13)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-SELF-HEALING.md (component_contracts AbcPromoter::retrain_and_promote + abc_promotion_diagram)</file>
  <file purpose="prior_task">TASK-HEAL-003 (struct shapes)</file>
  <file purpose="prior_task">TASK-HEAL-004 (HealReport, HealError, PromotionLockState)</file>
  <file purpose="prior_task">TASK-HEAL-005 (CF_MEJEPA_WEIGHT_BLOBS, CF_MEJEPA_ACTIVE_POINTERS, CF_MEJEPA_HEAL_REPORTS)</file>
  <file purpose="prior_task">TASK-HEAL-008 (SelfHealingPipeline orchestrator that owns the promotion_lock)</file>
  <file purpose="cross_domain_train">crates/context-graph-mejepa-train (Phase 3 train_from_scratch entry; CF_MEJEPA_MUTATION_CORPUS snapshot reader)</file>
  <file purpose="cross_domain_witness">crates/context-graph-witness/src/witness_chain.rs (WitnessChainAppender + ed25519 signing per SEC-MEJEPA-03; shipped)</file>
</input_context_files>

<prerequisites>
  <check>TASK-HEAL-003..008 complete</check>
  <check>Phase 3 training loop exposes `train_from_scratch(snapshot: &CorpusSnapshot, hyperparams: TrainingHyperparams) -> Result<ModelHandle, TrainingError>` (or HealError-mapped equivalent)</check>
  <check>HoldoutDataset loader reads from CF_MEJEPA_MUTATION_CORPUS holdout split (Phase 0 corpus shipped)</check>
  <check>WitnessChainAppender supports ModelPromote op_type (op_type registry exposes the variant)</check>
</prerequisites>

<scope>
  <in_scope>
    - Implement on `AbcPromoter`:
      - `pub fn retrain_and_promote(&mut self, trigger_reason: TriggerReason, predictor: Arc<RwLock<Predictor>>, storage: Arc<RocksDbStore>, witness_chain: Arc<WitnessChainAppender>, holdout: HoldoutDataset, lock: Arc<Mutex<PromotionLockState>>, calibration_version: &str) -> Result<HealReport, HealError>` per component_contracts (16-step body in technical spec)
      - Steps:
        1. `try_acquire_promotion_lock(lock, trigger_reason)`; if held, return Err(PromotionDeadlock { holder })
        2. Snapshot Mode A from current Arc&lt;RwLock&lt;Predictor&gt;&gt; (already in CF_MEJEPA_WEIGHT_BLOBS)
        3. Spawn Mode B: snapshot CF_MEJEPA_MUTATION_CORPUS at current_step (immutable, keyed by `corpus_sha + frozen_at`); call Phase 3 `train_from_scratch(snapshot)`; persist θ_B blob to CF_MEJEPA_WEIGHT_BLOBS keyed by `theta_sha`
        4. Spawn Mode C: grid-search w_A ∈ {0.0, 0.1, ..., 1.0} (11 points); per-weight evaluate ensemble on holdout via `compute_ensemble_score(w_A, theta_a, theta_b, holdout)`; pick (w_A, w_B=1-w_A) maximizing oracle_agreement; persist θ_C provenance (w_A, w_B, theta_a_sha, theta_b_sha)
        5. Evaluate all three on holdout (parallel via tokio spawn_blocking): per-mode HoldoutEval { coverage, oracle_agreement, ood_distribution_kl, num_samples, evaluation_summary_sha }
        6. `promotion_gate(...)`: a mode is promotable iff `coverage >= gate.stated_coverage_min && oracle_agreement >= mode_a.oracle_agreement && ood_distribution_kl <= mode_a.ood_distribution_kl`. If multiple pass, highest oracle_agreement wins. If none pass, return Err(HoldoutRegression { mode_a_score, mode_b_score, mode_c_score })
        7. Quota check: read RocksDB usage; if &gt; 90% of quota_gb (default 115 GB) return Err(QuotaExceeded { used_gb, quota_gb })
        8. Persist winner blob to CF_MEJEPA_WEIGHT_BLOBS keyed by theta_sha (deduplicated via dedupe-on-insert)
        9. Compute `evaluation_summary_sha = sha256(canonical_json([mode_a_eval, mode_b_eval, mode_c_eval]))`
        10. Append ModelPromote witness entry: `(timestamp, op_type=ModelPromote, weights_sha=theta_sha_winner, mode_winner, evaluation_summary_sha)` signed with ed25519 per SEC-MEJEPA-03
        11. Update CF_MEJEPA_ACTIVE_POINTERS["active_weights"] atomically (write_text_checked staging + fsync + atomic-rename to backing slot)
        12. Persist HealReport to CF_MEJEPA_HEAL_REPORTS keyed by timestamp_seconds
        13. Hot-swap predictor θ via Arc&lt;RwLock&lt;Predictor&gt;&gt; write lock (load weights_sha from blob; replace forward weights)
        14. Old weights retained for one promotion-cycle (Phase 6 GCs them; the previous active_weights theta_sha is recorded in HealReport)
        15. Release promotion_lock
        16. Return HealReport
    - Implement helpers:
      - `try_acquire_promotion_lock(lock, trigger_reason) -> Result<(), HealError>` — checks `held_by.is_none()`; sets `held_by = Some(thread_id_or_pid_string)`, `acquired_at = now()`, `trigger_reason = Some(trigger)`
      - `release_promotion_lock(lock)` — clears state
      - `compute_ensemble_score(w_a: f32, theta_a: &[f32], theta_b: &[f32], holdout: &HoldoutDataset) -> Result<f32, HealError>` — per-sample ensemble forward pass; oracle_agreement count
      - `evaluate_on_holdout(theta: &[f32], holdout: &HoldoutDataset, calibration_version: &str) -> Result<HoldoutEval, HealError>` — parallel batch forward via spawn_blocking
      - `promotion_gate(mode_a: &HoldoutEval, mode_b: &HoldoutEval, mode_c: &HoldoutEval, gate: &PromotionGate) -> Result<ModeWinner, HealError>` — pure decision function
      - `crash_recovery_scan_latest_promote(witness_chain: &WitnessChain, storage: &RocksDbStore) -> Result<(), HealError>` (per EC-HEAL-13) — scans for latest ModelPromote entry; verifies weights_sha is in CF_MEJEPA_WEIGHT_BLOBS; activates if active-pointer disagrees
    - Tests:
      - `try_acquire_lock_returns_deadlock_when_held`
      - `mode_b_train_from_scratch_persists_to_weight_blobs`
      - `mode_c_grid_search_picks_best_w_a`
      - `mode_c_grid_search_returns_w_a_one_when_a_dominates` (EC-HEAL-06)
      - `promotion_gate_promotes_b_when_b_beats_a_on_all_three`
      - `promotion_gate_no_winner_when_b_regresses_on_kl` (BR-HEAL-07 returns Err)
      - `promotion_gate_no_winner_returns_holdout_regression`
      - `quota_check_returns_quota_exceeded_at_90_percent` (EC-HEAL-05)
      - `witness_entry_appended_with_correct_op_type`
      - `witness_entry_signed_with_ed25519`
      - `evaluation_summary_sha_matches_canonical_json`
      - `active_pointer_updated_atomically_on_promotion`
      - `old_weights_retained_for_one_promotion_cycle`
      - `mode_b_nan_mid_retrain_returns_batch_nan_and_releases_lock` (EC-HEAL-05)
      - `concurrent_promotion_returns_promotion_deadlock` (EC-HEAL-12)
      - `crash_recovery_replays_latest_modelpromote_on_active_pointer_mismatch` (EC-HEAL-13)
      - End-to-end: synthetic Hard drift → A/B/C completes &lt; 60min budget on a small synthetic corpus (relax wallclock for unit tests; full assertion in TASK-HEAL-018 drill)
    - FSV evidence emission for TASK-HEAL-019: `AbcPromoter::fsv_snapshot() -> AbcFsvEvidence { mode_a_score, mode_b_score, mode_c_score, mode_c_weights, mode_winner, theta_sha_winner, evaluation_summary_sha, witness_chain_offset, promotion_latency_seconds }`
  </in_scope>
  <out_of_scope>
    - rollback_to (TASK-HEAL-010)
    - schedule_full_retrain (TASK-HEAL-011)
    - Phase 3 training loop internals
    - WitnessChain ModelPromote schema (assumed shipped; see depends_on)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/heal/promote.rs">
impl AbcPromoter {
    pub fn retrain_and_promote(
        &amp;mut self,
        trigger_reason: TriggerReason,
        predictor: Arc&lt;RwLock&lt;Predictor&gt;&gt;,
        storage: Arc&lt;RocksDbStore&gt;,
        witness_chain: Arc&lt;WitnessChainAppender&gt;,
        holdout: HoldoutDataset,
        lock: Arc&lt;Mutex&lt;PromotionLockState&gt;&gt;,
        calibration_version: &amp;str,
    ) -&gt; Result&lt;HealReport, HealError&gt;;

    pub fn fsv_snapshot(&amp;self) -&gt; AbcFsvEvidence;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AbcFsvEvidence {
    pub mode_a_score: HoldoutEval,
    pub mode_b_score: HoldoutEval,
    pub mode_c_score: HoldoutEval,
    pub mode_c_weights: (f32, f32),
    pub mode_winner: ModeWinner,
    pub theta_sha_winner: [u8; 32],
    pub evaluation_summary_sha: [u8; 32],
    pub witness_chain_offset: u64,
    pub promotion_latency_seconds: u64,
}
    </signature>
  </signatures>
  <constraints>
    - Promotion lock held for the ENTIRE 16-step call; released only on success or terminal error
    - All three modes evaluated against the SAME calibration version (snapshot at promotion start)
    - HoldoutDataset is immutable during promotion (acquired snapshot at start)
    - Atomic active-pointer update via write_text_checked (shipped, per implementation_notes)
    - Witness entry signed with ed25519 (SEC-MEJEPA-03); evaluation_summary_sha is sha256 of canonical JSON
    - No silent fallback to A when no mode passes 3 gates (returns Err per BR-HEAL-07)
    - Mode B retrain hyperparams: warm-start optimizer state from Mode A; reuse Phase 3 training loop bf16 AdamW + LoRA + InfoNCE + multitask
    - Mode C grid-search: 11 points (step 0.1) per implementation_notes
    - Wallclock budget: &lt;60min on RTX 5090 for default corpus (asserted in TASK-HEAL-018, NOT here)
    - File &lt; 500 lines (split into promote_holdout.rs if needed)
    - tracing::error! before every Err return per failure_logging note
  </constraints>
  <verification>
    - `cargo test -p context-graph-mejepa heal::promote` shows ≥ 14 new tests on top of TASK-HEAL-003 (= ≥ 28 total)
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - HoldoutRegression returned with all three HoldoutEval payloads
    - PromotionDeadlock returned on concurrent attempt
    - QuotaExceeded returned at &gt; 90% disk usage
    - WitnessChain entry persisted before active-pointer update (per atomic ordering)
    - On crash mid-call between witness entry and active-pointer update, recovery routine activates the latest ModelPromote (EC-HEAL-13)
  </verification>
</definition_of_done>

<pseudo_code>
pub fn retrain_and_promote(&mut self, trigger_reason, predictor, storage, witness_chain, holdout, lock, calibration_version) -> Result<HealReport, HealError> {
    let started_at = SystemTime::now();
    try_acquire_promotion_lock(&lock, trigger_reason)?;
    let result = (|| -> Result<HealReport, HealError> {
        // Mode A: snapshot
        let theta_a = predictor.read().unwrap().parameters_flat();
        let theta_a_sha = sha256(&theta_a);
        // Mode B: train_from_scratch
        let snapshot = snapshot_corpus_immutable(&storage, &corpus_sha)?;
        let theta_b = phase3_train::train_from_scratch(&snapshot, default_hyperparams())?;
        let theta_b_sha = sha256(&theta_b);
        storage.put(CF_MEJEPA_WEIGHT_BLOBS, &theta_b_sha, &theta_b)?;
        // Mode C: grid-search
        let mut best_score = -1.0_f32; let mut best_w_a = 1.0_f32;
        for k in 0..=10 {
            let w_a = k as f32 * 0.1;
            let score = compute_ensemble_score(w_a, &theta_a, &theta_b, &holdout)?;
            if score > best_score { best_score = score; best_w_a = w_a; }
        }
        // Holdout eval per mode
        let eval_a = evaluate_on_holdout(&theta_a, &holdout, calibration_version)?;
        let eval_b = evaluate_on_holdout(&theta_b, &holdout, calibration_version)?;
        let eval_c = evaluate_ensemble_on_holdout(best_w_a, &theta_a, &theta_b, &holdout, calibration_version)?;
        // Gate
        let gate = PromotionGate { stated_coverage_min: self.gate.stated_coverage_min, oracle_agreement_floor: eval_a.oracle_agreement, ood_kl_ceiling: eval_a.ood_distribution_kl, full_retrain_floor: self.gate.full_retrain_floor };
        let winner = promotion_gate(&eval_a, &eval_b, &eval_c, &gate)?; // returns Err(HoldoutRegression) on no winner
        // Quota check
        let usage_gb = storage.estimate_disk_gb()?;
        if usage_gb > 0.9 * 115.0 { return Err(HealError::QuotaExceeded { used_gb: usage_gb, quota_gb: 115.0 }); }
        // Persist + witness + active-pointer
        let theta_winner = match winner { ModeWinner::A => theta_a.clone(), ModeWinner::B => theta_b.clone(), ModeWinner::C => ensemble_combine(best_w_a, &theta_a, &theta_b), _ => unreachable!() };
        let theta_sha_winner = sha256(&theta_winner);
        storage.put(CF_MEJEPA_WEIGHT_BLOBS, &theta_sha_winner, &theta_winner)?;
        let eval_summary = serde_json::to_vec(&[eval_a, eval_b, eval_c])?;
        let evaluation_summary_sha = sha256(&eval_summary);
        let witness_offset = witness_chain.append_signed(WitnessOp::ModelPromote { weights_sha: theta_sha_winner, mode_winner: winner, evaluation_summary_sha })?;
        // Atomic active-pointer
        let active_value = ActivePointerValue { theta_sha_or_version: theta_sha_winner.to_vec(), frozen_at: now_secs() };
        storage.put_atomic(CF_MEJEPA_ACTIVE_POINTERS, b"active_weights", &bincode::serialize(&active_value)?)?;
        // Hot-swap
        predictor.write().unwrap().load_weights(&theta_winner)?;
        // Persist HealReport
        let report = HealReport::try_new(winner, eval_a, eval_b, eval_c, (best_w_a, 1.0 - best_w_a), theta_sha_winner, evaluation_summary_sha, witness_offset, started_at.elapsed().unwrap().as_secs(), StatusChange::Active, trigger_reason)?;
        storage.put(CF_MEJEPA_HEAL_REPORTS, &encode_heal_report_key(now_secs()), &bincode::serialize(&report)?)?;
        Ok(report)
    })();
    release_promotion_lock(&lock);
    result
}
</pseudo_code>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/heal/promote.rs">add retrain_and_promote body + helpers + AbcFsvEvidence + tests</file>
</files_to_modify>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/heal/promote_holdout.rs">evaluate_on_holdout + compute_ensemble_score helpers (split if size cap requires)</file>
</files_to_create>

<validation_criteria>
  <criterion>cargo test -p context-graph-mejepa heal::promote shows ≥ 14 new passing tests (≥ 28 total)</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>Concurrent retrain returns PromotionDeadlock with .code() == "MEJEPA_HEAL_PROMOTION_DEADLOCK"</criterion>
  <criterion>No mode passes 3 gates → HoldoutRegression with .code() == "MEJEPA_RETRAIN_HOLDOUT_REGRESSION"</criterion>
  <criterion>Disk &gt; 90% returns QuotaExceeded with .code() == "MEJEPA_RETRAIN_QUOTA_EXCEEDED"</criterion>
  <criterion>WitnessChain entry persisted with op_type=ModelPromote + ed25519 signature</criterion>
  <criterion>Active-pointer updated atomically AFTER witness entry</criterion>
  <criterion>NaN mid-Mode-B returns BatchNan and releases lock (EC-HEAL-05)</criterion>
  <criterion>Crash recovery replays latest ModelPromote on restart if active-pointer disagrees (EC-HEAL-13)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa heal::promote</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
