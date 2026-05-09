# TASK-EMBED-004 — LoraAdapterRegistration + VramBudgetReport + CalibrationCertificate + EmbedderRegistration

```xml
<task_spec id="TASK-EMBED-004" version="1.0">
<metadata>
  <title>Define persistence-bound data models: LoraAdapterRegistration, VramBudgetReport, CalibrationCertificate, EmbedderRegistration</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>4</sequence>
  <implements>
    <item ref="REQ-EMBED-02">EmbedderRegistration with weight_sha256 pin</item>
    <item ref="REQ-EMBED-05">VramBudgetReport for ≤29 GB enforcement</item>
    <item ref="REQ-EMBED-06">LoraAdapterRegistration with frozen-base SHA pre/post</item>
    <item ref="REQ-EMBED-09">CalibrationCertificate for E17 ed25519-signed gate</item>
    <item ref="TECH-EMBEDDERS::data_models">all four model definitions</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EMBED-003</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
These four data models persist into RocksDB column families (`CF_MEJEPA_EMBEDDER_REGISTRATIONS`, `CF_MEJEPA_LORA_ADAPTERS`, `CF_MEJEPA_CALIBRATION_CERTS`, `CF_MEJEPA_VRAM_REPORTS`). They MUST be serde-stable. `LoraAdapterRegistration` carries the `frozen_base_sha_pre` / `frozen_base_sha_post` invariant fields that BR-EMBED-04 enforces. `CalibrationCertificate` carries the ed25519 signature per SEC-MEJEPA-03 binding (corpus_sha, sample_count, calibration_method).
</context>

<input_context_files>
  <file purpose="data_models_spec">docs/ruvectorfindings/specs/technical/TECH-EMBEDDERS.md#data_models</file>
  <file purpose="security_rules">docs/ruvectorfindings/specs/constitution.md#security_requirements</file>
  <file purpose="ed25519_pattern">crates/context-graph-witness/src/cert.rs</file>
  <file purpose="embedder_id">crates/context-graph-mejepa-embedders/src/embedder_id.rs</file>
  <file purpose="types">crates/context-graph-mejepa-embedders/src/types.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-EMBED-003 complete (`EmbedderId`, `PrecisionClass` available)</check>
  <check>`ed25519-dalek` workspace dep available</check>
</prerequisites>

<scope>
  <in_scope>
    - `EmbedderRegistration { embedder_id, weight_files: Vec&lt;PathBuf&gt;, weight_sha256: String, version: String, vram_estimate_bytes: u64, latency_budget_ms: u32 }`
    - `LoraAdapterRegistration { adapter_id: Uuid, embedder_id, rank: usize, alpha: f32, dropout: f32, kind: AdapterKind, created_at: DateTime&lt;Utc&gt;, frozen_base_sha_pre: String, frozen_base_sha_post: String, parent_adapter_id: Option&lt;Uuid&gt; }`
    - `AdapterKind` enum: `Base` / `Delta`
    - `VramBudgetReport { resident_total_bytes, per_embedder_bytes: BTreeMap&lt;EmbedderId, u64&gt;, lora_adapters_bytes, cuda_driver_overhead_bytes, captured_at }`
    - `CalibrationCertificate { embedder_id, corpus_sha, sample_count, calibration_method, signed_at, signed_by_ed25519 }`
    - `CalibrationCertificate::sign(secret_key, ...) -&gt; Result&lt;CalibrationCertificate, EmbedError&gt;` and `verify(public_key) -&gt; Result&lt;(), EmbedError&gt;` per SEC-MEJEPA-03
    - All structs `serde::Serialize + Deserialize` with stable field names
    - Helper constructors for `VramBudgetReport::new()` and `LoraAdapterRegistration::new_base() / new_delta()`
    - Validators: `frozen_base_sha_pre` and `_post` MUST be 64 hex chars; `rank` ∈ [1, 256]; `alpha` ∈ (0.0, 1024.0]; `dropout` ∈ [0.0, 0.5]
    - `LoraAdapterRegistration::assert_base_unchanged() -&gt; Result&lt;(), EmbedError&gt;` returns `MEJEPA_EMBED_FROZEN_BASE_VIOLATION` if pre ≠ post
    - Unit tests: round-trip JSON byte-equal; ed25519 sign/verify happy path; tampered cert verify returns `MEJEPA_EMBED_E17_UNCALIBRATED`; assert_base_unchanged on equal/unequal SHAs
  </in_scope>
  <out_of_scope>
    - RocksDB CF wiring (TASK-EMBED-009 wires CF accessors via context-graph-storage)
    - Actual ed25519 keypair management (operator-controlled keyring; this task only signs/verifies given a key)
    - LoRA tensor allocation (TASK-EMBED-020)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-embedders/src/registration.rs">
      use std::collections::BTreeMap;
      use std::path::PathBuf;
      use chrono::{DateTime, Utc};
      use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
      use serde::{Deserialize, Serialize};
      use uuid::Uuid;

      use crate::EmbedderId;
      use crate::error::EmbedError;

      #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
      pub struct EmbedderRegistration {
          pub embedder_id: EmbedderId,
          pub weight_files: Vec&lt;PathBuf&gt;,
          pub weight_sha256: String,        // 64 hex
          pub version: String,
          pub vram_estimate_bytes: u64,
          pub latency_budget_ms: u32,
      }

      impl EmbedderRegistration {
          pub fn validate(&amp;self) -&gt; Result&lt;(), EmbedError&gt;;
      }

      #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
      #[serde(rename_all = "snake_case")]
      pub enum AdapterKind { Base, Delta }

      #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
      pub struct LoraAdapterRegistration {
          pub adapter_id: Uuid,
          pub embedder_id: EmbedderId,
          pub rank: usize,
          pub alpha: f32,
          pub dropout: f32,
          pub kind: AdapterKind,
          pub created_at: DateTime&lt;Utc&gt;,
          pub frozen_base_sha_pre: String,
          pub frozen_base_sha_post: String,
          pub parent_adapter_id: Option&lt;Uuid&gt;,
      }

      impl LoraAdapterRegistration {
          pub fn new_base(embedder_id: EmbedderId, frozen_base_sha: String, rank: usize, alpha: f32, dropout: f32) -&gt; Result&lt;Self, EmbedError&gt;;
          pub fn new_delta(embedder_id: EmbedderId, frozen_base_sha: String, rank: usize, alpha: f32, dropout: f32, parent_adapter_id: Uuid) -&gt; Result&lt;Self, EmbedError&gt;;
          pub fn validate(&amp;self) -&gt; Result&lt;(), EmbedError&gt;;
          pub fn assert_base_unchanged(&amp;self) -&gt; Result&lt;(), EmbedError&gt;;
      }

      #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
      pub struct VramBudgetReport {
          pub resident_total_bytes: u64,
          pub per_embedder_bytes: BTreeMap&lt;EmbedderId, u64&gt;,
          pub lora_adapters_bytes: u64,
          pub cuda_driver_overhead_bytes: u64,
          pub captured_at: DateTime&lt;Utc&gt;,
      }

      impl VramBudgetReport {
          pub fn new() -&gt; Self;
          pub fn validate(&amp;self) -&gt; Result&lt;(), EmbedError&gt;;
      }

      #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
      pub struct CalibrationCertificate {
          pub embedder_id: EmbedderId,
          pub corpus_sha: String,           // 64 hex
          pub sample_count: usize,
          pub calibration_method: String,
          pub signed_at: DateTime&lt;Utc&gt;,
          pub signed_by_ed25519: String,    // hex sig over (embedder_id, corpus_sha, sample_count, calibration_method)
      }

      impl CalibrationCertificate {
          pub fn sign(
              embedder_id: EmbedderId,
              corpus_sha: String,
              sample_count: usize,
              calibration_method: String,
              key: &amp;SigningKey,
          ) -&gt; Result&lt;Self, EmbedError&gt;;
          pub fn verify(&amp;self, key: &amp;VerifyingKey) -&gt; Result&lt;(), EmbedError&gt;;
          /// Canonical bytes signed: SHA-256 over big-endian-encoded
          /// `(embedder_id_slot_number, corpus_sha_bytes, sample_count_le_u64, calibration_method_utf8)`.
          fn canonical_message(&amp;self) -&gt; Vec&lt;u8&gt;;
      }
    </signature>
  </signatures>

  <constraints>
    - File &lt; 500 lines.
    - `frozen_base_sha_pre` / `_post` MUST be exactly 64 lowercase hex chars; reject otherwise.
    - `rank ∈ [1, 256]`; default 32 (per TECH-EMBEDDERS).
    - `alpha &gt; 0.0 &amp;&amp; alpha &lt;= 1024.0`; default 32.0.
    - `dropout ∈ [0.0, 0.5]`; default 0.05.
    - `sample_count ≥ 100` for CalibrationCertificate (E17 minimum per data_models).
    - `assert_base_unchanged()` returns `MEJEPA_EMBED_FROZEN_BASE_VIOLATION` if pre ≠ post.
    - `verify()` MUST fail closed with `MEJEPA_EMBED_E17_UNCALIBRATED` on signature mismatch (not panic, not unwrap).
    - `VramBudgetReport::resident_total_bytes ≤ 29 * 2^30` validation.
    - `per_embedder_bytes` sum ≤ resident_total_bytes (sanity check).
    - JSON serde stable (string field names, snake_case via `#[serde(rename_all = "snake_case")]`).
    - canonical_message uses big-endian / little-endian as documented; bytes serialized deterministically.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-embedders registration::tests` passes.
    - Round-trip JSON byte-equal for all 4 structs.
    - ed25519 sign + verify happy path passes; tampered `corpus_sha` byte → `verify()` returns `MEJEPA_EMBED_E17_UNCALIBRATED`.
    - `assert_base_unchanged()` accepts equal SHAs; rejects unequal with `MEJEPA_EMBED_FROZEN_BASE_VIOLATION`.
    - `VramBudgetReport::validate()` rejects total &gt; 29 GiB.
    - `LoraAdapterRegistration::new_base(rank=0, ...)` rejected.
  </verification>
</definition_of_done>

<pseudo_code>
registration.rs:
  - Four structs + AdapterKind enum.
  - Hex-validate helper checking 64 lowercase hex chars.
  - LoraAdapterRegistration::new_base/new_delta initialise with validated params and `frozen_base_sha_post = frozen_base_sha_pre` at construction (post is overwritten after refresh).
  - CalibrationCertificate::canonical_message: SHA-256 over concat of slot_number byte + corpus_sha bytes + sample_count.to_le_bytes() + calibration_method.as_bytes() — return as Vec&lt;u8&gt;.
  - sign(): SigningKey.sign(canonical_message()) → hex.
  - verify(): VerifyingKey.verify(canonical_message(), Signature::from_bytes(hex_decode(signed_by_ed25519))). Map any error to MEJEPA_EMBED_E17_UNCALIBRATED.
  - Tests: round-trips, tamper detection, validators reject malformed inputs.

lib.rs:
  - pub mod registration;
  - pub use registration::{EmbedderRegistration, LoraAdapterRegistration, AdapterKind, VramBudgetReport, CalibrationCertificate};
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-embedders/src/registration.rs">Four data models + ed25519 sign/verify + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-embedders/src/lib.rs">Add `pub mod registration;` + re-exports</file>
</files_to_modify>

<validation_criteria>
  <criterion>JSON round-trip byte-equal for all 4 structs</criterion>
  <criterion>ed25519 sign/verify happy + tamper paths covered</criterion>
  <criterion>assert_base_unchanged() returns FROZEN_BASE_VIOLATION on diff</criterion>
  <criterion>Validators reject out-of-range rank/alpha/dropout/sample_count</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-mejepa-embedders registration</command>
  <command>cargo clippy -p context-graph-mejepa-embedders --all-features -- -D warnings</command>
</test_commands>

</task_spec>
```
