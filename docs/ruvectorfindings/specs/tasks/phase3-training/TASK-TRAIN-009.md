# TASK-TRAIN-009 — Per-step training cert emission with parent_witness_hash → self_hash chain to CF_MEJEPA_TRAIN_CERTS

```xml
<task_spec id="TASK-TRAIN-009" version="1.0">

<metadata>
  <title>TrainCertWriter::emit (sorted-key JSON canonicalization → sha256 self_hash chained from prior parent_witness_hash → Merkle root over component sub-trees → RocksDB CF_MEJEPA_TRAIN_CERTS write keyed by step u64-big-endian + fsync every 100); chain replay verifier with byte-equal-readback assertion + tamper detection</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>9</sequence>
  <implements>
    <item ref="REQ-TRAIN-03">Per-step cert with hash-chain</item>
    <item ref="AC-TRAIN-03-01..05">Schema; chain link; replay; skip_reason; target_collapse==0 invariant scan</item>
    <item ref="BR-TRAIN-03">Cert emission rules</item>
    <item ref="EC-TRAIN-05">target_collapse&gt;0 → MEJEPA_INSTRUMENT_GRADIENT_LEAK halt</item>
    <item ref="EC-TRAIN-06">parent_hash mismatch → MEJEPA_TRAIN_CERT_CHAIN_BROKEN</item>
    <item ref="ERR-TRAIN-05">CERT_CHAIN_BROKEN</item>
    <item ref="ERR-TRAIN-07">INSTRUMENT_GRADIENT_LEAK</item>
    <item ref="component:TrainCertWriter">crates/context-graph-mejepa-train/src/cert/*</item>
  </implements>
  <depends_on>
    <task_ref>TASK-TRAIN-001</task_ref>
    <task_ref>TASK-TRAIN-002</task_ref>
    <task_ref>../utml-learning-formula/TASK-UTML-007</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>3 created</estimated_files>
</metadata>

<context>
The per-step training cert is the physical M(t) consolidation tensor (UTML §XII). Every gradient update produces a JSON-serialized cert with deterministic sorted keys, hashed with sha256, chained via parent_witness_hash → self_hash, with a Merkle root over component sub-trees for efficient partial verification. Persisted to CF_MEJEPA_TRAIN_CERTS keyed by step in big-endian for ordered iteration. fsync every 100 to bound durability gap. Auditor (UT-05) replays the chain end-to-end on a 100,000-step run within 60 seconds.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (testing_requirements: 100k-entry chain replay; failure_taxonomy MEJEPA_TRAIN_CERT_CHAIN_BROKEN, MEJEPA_INSTRUMENT_GRADIENT_LEAK)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-TRAINING.md (US-TRAIN-03; AC-TRAIN-03-01..05; BR-TRAIN-03; EC-TRAIN-05..06; ERR-TRAIN-05, 07)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-TRAINING.md (data_models TrainingCertificate full schema; cert/* paths; rocksdb_column_families CF_MEJEPA_TRAIN_CERTS)</file>
  <file purpose="prior_tasks">TASK-TRAIN-001 (TrainCertWriter struct); TASK-TRAIN-002 (TrainerError); ../utml-learning-formula/TASK-UTML-001 (TrainingCertificate base struct), TASK-UTML-007 (compute_l_step + components)</file>
  <file purpose="shipped_code">crates/context-graph-witness (SHAKE-256-73 + Merkle helpers)</file>
</input_context_files>

<prerequisites>
  <check>TASK-TRAIN-001 + TASK-TRAIN-002 + UTML-001..007 complete</check>
  <check>RocksDB CF_MEJEPA_TRAIN_CERTS column family registered (TASK-TRAIN-001 captures the name)</check>
</prerequisites>

<scope>
  <in_scope>
    - Modify `src/cert/mod.rs`: extend `TrainingCertificate` with Phase-3 specific fields per TECH-TRAINING data_models (loss_components, training_mode, distillation_cycle_id, distillation_loss_mean, distillation_skipped_count, counterfactual_smoothness, counterfactual_smoothness_anomaly, cross_task_transfer_indices, cross_task_transfer_fallback_count, holdout_promotion, generic_only_warning, phase3_dod_passed, grad_norm_running_mean) — all Option where appropriate
    - Create `src/cert/chain.rs` with:
      - `pub fn body_canonical_json(cert: &amp;TrainingCertificate) -&gt; Result&lt;String, TrainerError&gt;` (sorted keys; deterministic float formatting via fixed-precision serializer)
      - `pub fn compute_self_hash(canonical_body: &amp;str) -&gt; String` (sha256 hex)
      - `pub fn compute_parent_hash(prev_self_hash: &amp;str, prev_canonical_body: &amp;str) -&gt; String` (sha256(prev_self_hash || prev_canonical_body))
      - `pub fn compute_merkle_root(components: &amp;HashMap&lt;String, f32&gt;) -&gt; String` (build Merkle tree over component sub-trees)
      - `pub fn verify_chain(rocksdb: &amp;DB, cf_name: &amp;str, from_step: u64, to_step: u64) -&gt; Result&lt;ChainVerificationReport, TrainerError&gt;`
      - `pub struct ChainVerificationReport { pub from: u64, pub to: u64, pub verified: u64, pub broken_at: Option&lt;u64&gt; }`
    - Create `src/cert/invariant_scan.rs` with:
      - `pub fn scan_target_collapse_invariant(rocksdb: &amp;DB, cf_name: &amp;str, from: u64, to: u64) -&gt; Result&lt;Vec&lt;u64&gt;, TrainerError&gt;` returns step indices where target_collapse != 0.0 (should always be empty)
    - Implement `TrainCertWriter::emit(&amp;mut self, cert: TrainingCertificate) -&gt; Result&lt;(), TrainerError&gt;`:
      1. Verify cert.signal.delta_xi_components.target_collapse == 0.0 — emit MejepaInstrumentGradientLeak otherwise
      2. Compute body_canonical_json
      3. cert.parent_witness_hash = self.last_self_hash (initialized to 32-byte zero hex on first cert)
      4. cert.self_hash = sha256(canonical_body)
      5. cert.merkle_root = merkle_root(cert.signal.components map)
      6. RocksDB put on CF_MEJEPA_TRAIN_CERTS with key = step.to_be_bytes(), value = canonical JSON bytes
      7. self.last_self_hash = cert.self_hash; cert_count_since_fsync += 1
      8. If count_since_fsync &gt;= 100: rocksdb.flush() + reset counter
    - Implement `TrainCertWriter::resume_from_rocksdb(rocksdb: Arc&lt;DB&gt;, cf_name: String) -&gt; Result&lt;Self, TrainerError&gt;` reading the last cert's self_hash to chain off
    - Tamper test in unit tests: corrupt cert[N].body bytes → verify_chain detects mismatch at N+1 with MejepaTrainCertChainBroken
    - Performance test: write 100,000 synthetic certs to a tmpfs RocksDB; verify_chain replay completes &lt; 60s
  </in_scope>
  <out_of_scope>
    - ed25519 signing of certs (per-epoch witness chain handles signing; per-step chain uses sha256 only per TECH-TRAINING)
    - HoldoutReport persistence (TASK-TRAIN-011)
    - Trainer.emit_cert orchestration call (TASK-TRAIN-012)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/src/cert/chain.rs">
use rocksdb::DB;
use std::collections::HashMap;
use crate::cert::TrainingCertificate;
use crate::error::{TrainerError, TrainerErrorCode};

pub const FSYNC_INTERVAL: u64 = 100;
pub const GENESIS_PARENT_HASH: &amp;str = "0000000000000000000000000000000000000000000000000000000000000000";

pub fn body_canonical_json(cert: &amp;TrainingCertificate) -&gt; Result&lt;String, TrainerError&gt;;
pub fn compute_self_hash(canonical_body: &amp;str) -&gt; String;
pub fn compute_parent_hash(prev_self_hash: &amp;str, prev_canonical_body: &amp;str) -&gt; String;
pub fn compute_merkle_root(components: &amp;HashMap&lt;String, f32&gt;) -&gt; String;

#[derive(Debug, Clone)]
pub struct ChainVerificationReport {
    pub from: u64,
    pub to: u64,
    pub verified: u64,
    pub broken_at: Option&lt;u64&gt;,
}

pub fn verify_chain(rocksdb: &amp;DB, cf_name: &amp;str, from: u64, to: u64) -&gt; Result&lt;ChainVerificationReport, TrainerError&gt;;
    </signature>
    <signature file="crates/context-graph-mejepa-train/src/cert/invariant_scan.rs">
use rocksdb::DB;
use crate::error::TrainerError;

pub fn scan_target_collapse_invariant(rocksdb: &amp;DB, cf_name: &amp;str, from: u64, to: u64) -&gt; Result&lt;Vec&lt;u64&gt;, TrainerError&gt;;
    </signature>
    <signature file="crates/context-graph-mejepa-train/src/cert/writer.rs">
impl TrainCertWriter {
    pub fn new(
        rocksdb: Arc&lt;DB&gt;,
        cf_name: String,
        code_version: String,
        corpus_sha: String,
        embedder_versions: HashMap&lt;String, String&gt;,
        frozen_at: String,
    ) -&gt; Result&lt;Self, TrainerError&gt;;

    pub fn resume_from_rocksdb(
        rocksdb: Arc&lt;DB&gt;,
        cf_name: String,
        code_version: String,
        corpus_sha: String,
        embedder_versions: HashMap&lt;String, String&gt;,
        frozen_at: String,
    ) -&gt; Result&lt;Self, TrainerError&gt;;

    pub fn emit(&amp;mut self, cert: &amp;mut TrainingCertificate) -&gt; Result&lt;(), TrainerError&gt;;

    pub fn last_self_hash(&amp;self) -&gt; &amp;str;
}
    </signature>
  </signatures>
  <constraints>
    - body_canonical_json: serde_json::Value → walk + sort all map keys recursively → serialize with fixed-precision floats (use `serde_json::ser::PrettyFormatter` disabled or custom; or `to_string` on a sorted-Value)
    - sha256 via `sha2 = "0.10"` (add to Cargo.toml)
    - Genesis parent: 32 bytes of 0x00 hex (64-char string)
    - Merkle root: build binary tree over sorted component keys; pad to power of 2 with zero-leaves
    - rocksdb key: step.to_be_bytes() (8 bytes)
    - target_collapse check happens BEFORE write — fail-closed prevents poisoned chain from ever persisting
    - fsync every FSYNC_INTERVAL writes
    - verify_chain iterates rocksdb in step-order; rebuilds canonical body; recomputes self_hash; compares parent_witness_hash of next entry; first mismatch returns ChainVerificationReport.broken_at = Some(step) AND emits MejepaTrainCertChainBroken
    - Files &lt; 500 lines each
  </constraints>
  <verification>
    - cargo test ≥ 12 tests pass: emit + verify roundtrip; tamper test detects break; target_collapse&gt;0 fail-closed; sorted-key determinism; Merkle root stable; genesis hash 0x0...0; fsync interval; resume_from_rocksdb reads correctly
    - Performance test: synthetic 100,000-cert chain replay under 60s on dev machine
  </verification>
</definition_of_done>

<pseudo_code>
pub fn body_canonical_json(cert) -&gt; Result&lt;String, TrainerError&gt; {
    let v = serde_json::to_value(cert)?;
    let sorted = sort_keys_recursive(v);
    serde_json::to_string(&amp;sorted).map_err(|e| TrainerError::new(MejepaTrainCertChainBroken, format!("canonicalization: {e}")))
}

impl TrainCertWriter {
    pub fn emit(&amp;mut self, cert: &amp;mut TrainingCertificate) -&gt; Result&lt;(), TrainerError&gt; {
        // 1. invariant gate
        if cert.signal.delta_xi_components.target_collapse != 0.0 {
            return Err(TrainerError::new(MejepaInstrumentGradientLeak,
                format!("target_collapse={} at step {}", cert.signal.delta_xi_components.target_collapse, cert.step))
                .with_step(cert.step)
                .with_context(json!({"target_collapse": cert.signal.delta_xi_components.target_collapse})));
        }
        // 2. fill chain fields
        cert.parent_witness_hash = self.last_self_hash.clone();
        let body = body_canonical_json(cert)?;
        cert.self_hash = compute_self_hash(&amp;body);
        cert.merkle_root = compute_merkle_root(&amp;cert.signal.components_as_map());
        // 3. re-canonicalize with chain fields populated
        let final_body = body_canonical_json(cert)?;
        // 4. rocksdb put
        let cf = self.rocksdb.cf_handle(&amp;self.cf_name).ok_or_else(|| TrainerError::new(MejepaTrainConfigInvalid, "missing CF"))?;
        self.rocksdb.put_cf(&amp;cf, cert.step.to_be_bytes(), final_body.as_bytes())?;
        self.last_self_hash = cert.self_hash.clone();
        self.cert_count_since_fsync += 1;
        if self.cert_count_since_fsync &gt;= FSYNC_INTERVAL {
            self.rocksdb.flush()?;
            self.cert_count_since_fsync = 0;
        }
        Ok(())
    }
}

pub fn verify_chain(db, cf, from, to) -&gt; Result&lt;ChainVerificationReport, TrainerError&gt; {
    let cf = db.cf_handle(cf).ok_or(...)?;
    let mut prev_hash = GENESIS_PARENT_HASH.to_string();
    let mut verified = 0u64;
    for step in from..=to {
        let bytes = db.get_cf(&amp;cf, step.to_be_bytes())?.ok_or_else(|| TrainerError::new(MejepaTrainCertChainBroken, format!("missing cert step {step}")))?;
        let cert: TrainingCertificate = serde_json::from_slice(&amp;bytes)?;
        if cert.parent_witness_hash != prev_hash {
            return Ok(ChainVerificationReport { from, to, verified, broken_at: Some(step) });
        }
        let body = body_canonical_json(&amp;cert)?;
        let recomputed = compute_self_hash(&amp;body);
        if recomputed != cert.self_hash {
            return Ok(ChainVerificationReport { from, to, verified, broken_at: Some(step) });
        }
        prev_hash = cert.self_hash;
        verified += 1;
    }
    Ok(ChainVerificationReport { from, to, verified, broken_at: None })
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/src/cert/chain.rs">canonicalization + sha256 + Merkle + verify_chain + tests</file>
  <file path="crates/context-graph-mejepa-train/src/cert/invariant_scan.rs">scan_target_collapse_invariant + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-train/src/cert/mod.rs">extend TrainingCertificate with 13 Phase-3 fields</file>
  <file path="crates/context-graph-mejepa-train/src/cert/writer.rs">implement new + resume + emit methods</file>
  <file path="crates/context-graph-mejepa-train/Cargo.toml">add sha2 0.10 dep</file>
</files_to_modify>

<validation_criteria>
  <criterion>target_collapse != 0 fails closed BEFORE rocksdb write</criterion>
  <criterion>Genesis parent hash is 64-char "0x00" string</criterion>
  <criterion>verify_chain: 100-cert synthetic chain → 100 verified, no break</criterion>
  <criterion>Tamper test: modify cert[50].body → broken_at == Some(51) (chain check at next step)</criterion>
  <criterion>Sorted-key canonicalization deterministic across runs</criterion>
  <criterion>fsync every 100 certs</criterion>
  <criterion>100k-cert chain replay &lt; 60s</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train</command>
  <command>cargo test -p context-graph-mejepa-train --lib cert</command>
  <command>cargo test -p context-graph-mejepa-train --release --lib cert::chain::tests::stress_100k</command>
  <command>cargo clippy -p context-graph-mejepa-train -- -D warnings</command>
</test_commands>

</task_spec>
```
