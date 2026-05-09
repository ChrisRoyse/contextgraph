# TASK-TRAIN-010 — Per-epoch SHAKE-256-73 witness chain entry + ed25519 signing

```xml
<task_spec id="TASK-TRAIN-010" version="1.0">

<metadata>
  <title>EpochWitnessChain::append: aggregate per-epoch summary, canonicalize, hash via SHAKE-256-73 (chained from prior epoch), ed25519-sign over (parent_hash || self_hash || epoch), persist 73-byte raw bytes to CF_MEJEPA_EPOCH_WITNESS keyed by epoch u32-big-endian; replay verifier</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>10</sequence>
  <implements>
    <item ref="REQ-TRAIN-09">Per-epoch 73-byte SHAKE-256 witness chain entry</item>
    <item ref="AC-TRAIN-09-01..02">EpochSummary canonicalized + chained; 10-entry × 73B replay &lt; 5s</item>
    <item ref="BR-TRAIN-09">Genesis epoch 0 has zero parent</item>
    <item ref="NFR-TRAIN-07">ed25519 signature on every entry</item>
    <item ref="component:EpochWitnessChain">crates/context-graph-mejepa-train/src/eval/mod.rs (struct from TASK-TRAIN-001)</item>
  </implements>
  <depends_on>
    <task_ref>TASK-TRAIN-009</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created</estimated_files>
</metadata>

<context>
The per-epoch witness chain is a compact 73-bytes-per-epoch audit summary chained from the prior epoch's hash and signed with the operator's ed25519 keypair. 10 epochs × 73 bytes = 730 bytes total, replayable in &lt; 5 seconds. Layout (per TECH-TRAINING data_models.EpochWitnessEntry): bytes[0..32]=parent_hash; [32..64]=self_hash; [64..72]=epoch_u64_be; [72..73]=version_byte. The 64-byte ed25519 signature is stored alongside (or appended; spec requires 73-byte raw entry + signature retrievable; the layout doc says 73 bytes — interpret as 73-byte witness body PLUS separate 64-byte sig in a wrapper struct).
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (security_requirements SEC-MEJEPA-03 ed25519 signing; tech_stack sha3 0.7 SHAKE-256)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-TRAINING.md (US-TRAIN-09; AC-TRAIN-09-01..02; BR-TRAIN-09; NFR-TRAIN-07)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-TRAINING.md (data_models EpochWitnessEntry layout; CF_MEJEPA_EPOCH_WITNESS)</file>
  <file purpose="prior_tasks">TASK-TRAIN-001 (EpochWitnessChain struct), TASK-TRAIN-009 (cert chain pattern reused for canonicalization)</file>
  <file purpose="shipped_code">crates/context-graph-witness/src/cert.rs (Stage 0 + Stage F + Merkle SHAKE-256 73-byte primitive)</file>
</input_context_files>

<prerequisites>
  <check>TASK-TRAIN-001 + TASK-TRAIN-009 complete</check>
  <check>context-graph-witness crate provides 73-byte SHAKE-256 primitive (✅ shipped)</check>
  <check>ed25519-dalek 2.x in workspace</check>
</prerequisites>

<scope>
  <in_scope>
    - Implement methods on `EpochWitnessChain` (struct from TASK-TRAIN-001):
      - `pub fn new(rocksdb: Arc&lt;DB&gt;, cf_name: String, signing_key: SigningKey) -&gt; Self` (last_epoch_hash = [0u8; 32] for genesis)
      - `pub fn resume(rocksdb: Arc&lt;DB&gt;, cf_name: String, signing_key: SigningKey) -&gt; Result&lt;Self, TrainerError&gt;` reads last entry to recover last_epoch_hash
      - `pub fn append(&amp;mut self, summary: EpochSummary) -&gt; Result&lt;EpochWitnessEntry, TrainerError&gt;`
      - `pub fn verify(&amp;self) -&gt; Result&lt;EpochWitnessReplay, TrainerError&gt;` replays full chain
    - append behavior:
      1. Canonicalize EpochSummary (sorted-key JSON)
      2. SHAKE-256 of canonical body → 32-byte self_hash (use sha3 crate's SHAKE-256 with 32-byte output)
      3. parent_hash = self.last_epoch_hash
      4. Build 73-byte body: bytes[0..32]=parent_hash; [32..64]=self_hash; [64..72]=epoch.to_be_bytes() (u64 BE in 8 bytes); [72..73]=version_byte (1)
      5. ed25519 signature over (parent_hash || self_hash || epoch_u64_be) using SigningKey
      6. Construct EpochWitnessEntry { bytes: [..73 raw..], parent_witness_hash, self_hash, ed25519_signature, layout: descriptive String }
      7. RocksDB put on CF_MEJEPA_EPOCH_WITNESS key=epoch.to_be_bytes() (4 bytes — epoch is u32; though TECH-TRAINING also says u32_be)
      8. Persist 73-byte raw + 64-byte sig (137 total bytes per entry value); update last_epoch_hash
    - verify replay:
      - Iterate epoch=0..N: read entry; verify parent_hash chains; verify ed25519 signature against operator's verifying key; recompute self_hash from canonical EpochSummary; mismatch → return EpochWitnessReplay { broken_at: Some(epoch) }
    - Use SigningKey::generate via `rand` for tests; production uses operator-controlled keyring (out of scope for this task, but document)
    - Unit tests: 10-epoch chain → verify; tamper test detects break; genesis epoch 0 parent = zero; signature verification round-trip
  </in_scope>
  <out_of_scope>
    - Operator keyring management (Phase 7 secrets management)
    - Trainer call to append (TASK-TRAIN-012)
    - HoldoutReport persistence (TASK-TRAIN-011)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/src/eval/epoch_witness.rs">
use std::sync::Arc;
use rocksdb::DB;
use ed25519_dalek::{SigningKey, VerifyingKey, Signer, Signature};
use sha3::{Shake256, digest::{Update, ExtendableOutput, XofReader}};
use crate::error::{TrainerError, TrainerErrorCode};
use crate::eval::{EpochSummary, EpochWitnessEntry};

pub const ENTRY_BODY_LEN: usize = 73;
pub const ENTRY_SIG_LEN: usize = 64;
pub const ENTRY_VERSION_BYTE: u8 = 1;

pub struct EpochWitnessChain {
    pub rocksdb: Arc&lt;DB&gt;,
    pub cf_epoch_witness_name: String,
    pub last_epoch_hash: [u8; 32],
    pub ed25519_signing_key: SigningKey,
}

impl EpochWitnessChain {
    pub fn new(rocksdb: Arc&lt;DB&gt;, cf_name: String, signing_key: SigningKey) -&gt; Self;
    pub fn resume(rocksdb: Arc&lt;DB&gt;, cf_name: String, signing_key: SigningKey) -&gt; Result&lt;Self, TrainerError&gt;;
    pub fn append(&amp;mut self, summary: EpochSummary) -&gt; Result&lt;EpochWitnessEntry, TrainerError&gt;;
    pub fn verify(&amp;self, verifying_key: &amp;VerifyingKey) -&gt; Result&lt;EpochWitnessReplay, TrainerError&gt;;
}

#[derive(Debug, Clone)]
pub struct EpochWitnessReplay {
    pub epochs_verified: u32,
    pub broken_at: Option&lt;u32&gt;,
}

pub fn shake256_32(data: &amp;[u8]) -&gt; [u8; 32];
    </signature>
  </signatures>
  <constraints>
    - SHAKE-256 with 32-byte output via sha3 crate (`Shake256` + XofReader read 32 bytes)
    - ed25519 signature over `parent_hash[32] || self_hash[32] || epoch.to_be_bytes()[8]` = 72 bytes input
    - Genesis: epoch=0 → parent_hash = [0u8; 32]
    - 73-byte body layout fixed
    - Storage layout: 73 + 64 = 137 bytes per epoch entry value (body || sig)
    - File &lt; 350 lines
  </constraints>
  <verification>
    - cargo test ≥ 6 tests pass: 10-epoch chain replay; tamper test; genesis parent zero; signature round-trip; SHAKE-256 vector test
  </verification>
</definition_of_done>

<pseudo_code>
pub fn shake256_32(data: &amp;[u8]) -&gt; [u8; 32] {
    let mut h = Shake256::default(); h.update(data);
    let mut out = [0u8; 32]; h.finalize_xof().read(&amp;mut out); out
}

impl EpochWitnessChain {
    pub fn append(&amp;mut self, s: EpochSummary) -&gt; Result&lt;EpochWitnessEntry, TrainerError&gt; {
        let canon = body_canonical_json(&amp;s)?; // sorted-key JSON of EpochSummary
        let self_hash = shake256_32(canon.as_bytes());
        let parent = self.last_epoch_hash;
        let mut body = [0u8; ENTRY_BODY_LEN];
        body[0..32].copy_from_slice(&amp;parent);
        body[32..64].copy_from_slice(&amp;self_hash);
        body[64..72].copy_from_slice(&amp;(s.epoch as u64).to_be_bytes());
        body[72] = ENTRY_VERSION_BYTE;
        let sig = self.ed25519_signing_key.sign(&amp;body[..72]).to_bytes();
        let mut blob = Vec::with_capacity(ENTRY_BODY_LEN + ENTRY_SIG_LEN);
        blob.extend_from_slice(&amp;body); blob.extend_from_slice(&amp;sig);
        let cf = self.rocksdb.cf_handle(&amp;self.cf_epoch_witness_name).ok_or(...)?;
        self.rocksdb.put_cf(&amp;cf, s.epoch.to_be_bytes(), blob)?;
        self.last_epoch_hash = self_hash;
        Ok(EpochWitnessEntry { bytes: body.to_vec(), parent_witness_hash: parent, self_hash, ed25519_signature: sig, layout: "...".into() })
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/src/eval/epoch_witness.rs">EpochWitnessChain methods + shake256_32 + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-train/src/eval/mod.rs">pub mod epoch_witness;</file>
  <file path="crates/context-graph-mejepa-train/Cargo.toml">add sha3 0.10 dep (or via context-graph-witness re-export)</file>
</files_to_modify>

<validation_criteria>
  <criterion>10-epoch chain → EpochWitnessReplay.epochs_verified == 10, broken_at == None</criterion>
  <criterion>Tamper test on epoch 5 → broken_at == Some(5)</criterion>
  <criterion>Genesis (epoch 0) parent_hash all zeros</criterion>
  <criterion>ed25519 signature verifies with VerifyingKey from same SigningKey</criterion>
  <criterion>73-byte body layout exact</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train</command>
  <command>cargo test -p context-graph-mejepa-train --lib epoch_witness</command>
  <command>cargo clippy -p context-graph-mejepa-train -- -D warnings</command>
</test_commands>

</task_spec>
```
