# TASK-EMBED-008 — Streaming SHA-256 weight digest verifier

```xml
<task_spec id="TASK-EMBED-008" version="1.0">
<metadata>
  <title>Streaming SHA-256 over weight files in 4 MB chunks; multi-shard manifest digest; pinned models_config.toml verification</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>8</sequence>
  <implements>
    <item ref="REQ-EMBED-02">SHA-256 digest pinning per SEC-MEJEPA-02</item>
    <item ref="BR-EMBED-01">refuse loading on digest mismatch</item>
    <item ref="TECH-EMBEDDERS::implementation_notes (security)">streaming SHA-256 in 4 MB chunks</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EMBED-002</task_ref>
    <task_ref>TASK-EMBED-005</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>2 files</estimated_files>
</metadata>

<context>
Constitution `SEC-MEJEPA-02` mandates SHA-256 digest pinning of every embedder weight file. Some embedders ship 5+ GB weights (E7 Qodo at ~5.8 GB fp32) — full file load into RAM is unsafe. This module streams the file in 4 MB chunks via `sha2::Sha256::update`. For multi-shard weights (`shard-0.safetensors`, `shard-1.safetensors`), digest a sorted-by-filename manifest concatenation. Loads `config/models_config.toml` and provides per-EmbedderId digest lookup.
</context>

<input_context_files>
  <file purpose="security_rule">docs/ruvectorfindings/specs/constitution.md (SEC-MEJEPA-02)</file>
  <file purpose="digest_spec">docs/ruvectorfindings/specs/technical/TECH-EMBEDDERS.md#implementation_notes (security)</file>
  <file purpose="error_taxonomy">crates/context-graph-mejepa-embedders/src/error.rs</file>
  <file purpose="registration">crates/context-graph-mejepa-embedders/src/registration.rs</file>
  <file purpose="embedder_id">crates/context-graph-mejepa-embedders/src/embedder_id.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-EMBED-002 (`EmbedderId`)</check>
  <check>TASK-EMBED-004 (`EmbedderRegistration`)</check>
  <check>TASK-EMBED-005 (`EmbedError::DigestMismatch`)</check>
  <check>sha2 workspace dep available</check>
</prerequisites>

<scope>
  <in_scope>
    - `pub fn streaming_sha256<R: Read>(reader: R) -> EmbedResult<[u8; 32]>` — 4 MB chunks
    - `pub fn streaming_sha256_file(path: &Path) -> EmbedResult<[u8; 32]>` — opens file then streams
    - `pub fn manifest_sha256(paths: &[PathBuf]) -> EmbedResult<[u8; 32]>` — sorts paths by filename, hashes `(filename || file_sha)*` concatenation
    - `pub fn hex_digest(digest: [u8; 32]) -> String` — 64 lowercase hex
    - `pub fn parse_hex_digest(hex: &str) -> EmbedResult<[u8; 32]>` — validates 64 hex chars, returns InvalidInput on malformed
    - `pub struct ModelsConfig { embedders: BTreeMap<EmbedderId, EmbedderRegistration> }`
    - `pub fn load_models_config(path: &Path) -> EmbedResult<ModelsConfig>` — TOML parse with serde
    - `pub fn verify_embedder_digest(reg: &EmbedderRegistration) -> EmbedResult<()>` — runs streaming digest over `weight_files`, compares to `weight_sha256` pin, returns `MEJEPA_EMBED_DIGEST_MISMATCH` on mismatch
    - Tests:
      - 4 MB-aligned + non-aligned synthetic file digests match `sha2::Sha256::digest(full_bytes)` reference
      - manifest digest with [a, b, c] paths byte-equal to manifest with [c, a, b] paths (sort-canonicalized)
      - parse_hex_digest rejects 63-char and 65-char and non-hex
      - models_config.toml round-trip: write → load → equal
      - verify_embedder_digest happy path passes
      - tampered byte → DigestMismatch with `expected_sha`, `actual_sha`, `file_path` populated
  </in_scope>
  <out_of_scope>
    - Calling verify from Ensemble::load_one (TASK-EMBED-009 wires it)
    - Real production weight files (CI uses synthetic fixture files via tempfile)
    - GPG/ed25519 signing of `models_config.toml` itself (auditability via git per spec)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-embedders/src/digest.rs">
      use std::collections::BTreeMap;
      use std::fs::File;
      use std::io::{BufReader, Read};
      use std::path::{Path, PathBuf};
      use sha2::{Digest, Sha256};

      use crate::EmbedderId;
      use crate::error::{EmbedError, EmbedResult};
      use crate::registration::EmbedderRegistration;

      pub const STREAMING_CHUNK_BYTES: usize = 4 * 1024 * 1024; // 4 MiB

      pub fn streaming_sha256&lt;R: Read&gt;(reader: R) -&gt; EmbedResult&lt;[u8; 32]&gt;;
      pub fn streaming_sha256_file(path: &amp;Path) -&gt; EmbedResult&lt;[u8; 32]&gt;;
      pub fn manifest_sha256(paths: &amp;[PathBuf]) -&gt; EmbedResult&lt;[u8; 32]&gt;;
      pub fn hex_digest(digest: [u8; 32]) -&gt; String;
      pub fn parse_hex_digest(hex: &amp;str) -&gt; EmbedResult&lt;[u8; 32]&gt;;

      #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
      pub struct ModelsConfig {
          pub embedders: BTreeMap&lt;EmbedderId, EmbedderRegistration&gt;,
      }

      pub fn load_models_config(path: &amp;Path) -&gt; EmbedResult&lt;ModelsConfig&gt;;
      pub fn verify_embedder_digest(reg: &amp;EmbedderRegistration) -&gt; EmbedResult&lt;()&gt;;
    </signature>
    <signature file="config/models_config.toml.example">
      # Example config (real config in config/models_config.toml; populate as
      # weights are downloaded and verified). Per SEC-MEJEPA-02 the digest
      # pin is the source of truth — wrong digest refuses load fail-closed.

      [embedders.E1]
      weight_files = ["models/e5-large-v2/pytorch_model.bin"]
      weight_sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
      version = "e5-large-v2-2024-11-15"
      vram_estimate_bytes = 681574400  # ~650 MB bf16
      latency_budget_ms = 5

      [embedders.E5]
      weight_files = ["models/nomic-embed-text-v1.5/pytorch_model.bin"]
      weight_sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
      version = "nomic-embed-text-v1.5"
      vram_estimate_bytes = 304087040
      latency_budget_ms = 4

      # ... entries for E7, E10, E11, E14, E17 minimum for Phase 1b
    </signature>
  </signatures>

  <constraints>
    - File `digest.rs` &lt; 500 lines.
    - Streaming hash uses `BufReader` + a 4 MiB stack-or-heap buffer; never `std::fs::read` full file.
    - manifest_sha256: paths cloned then sorted by filename (`Path::file_name()`); for each path, hash `(filename_utf8 || ":" || hex_digest_of_file)`; final hash is SHA-256 of that concatenation.
    - hex_digest produces lowercase hex (64 chars); parse_hex_digest accepts only lowercase hex (constitution provenance: deterministic byte representation).
    - load_models_config returns InvalidInput on malformed TOML; uses `toml` crate (workspace dep — add to Cargo.toml in this task if absent).
    - verify_embedder_digest: if `weight_files.len() == 1` use streaming_sha256_file, else manifest_sha256; compare against `weight_sha256` after parse_hex_digest.
    - DigestMismatch error includes embedder_id (slot_number), expected_sha, actual_sha, file_path (first file or manifest path placeholder).
    - NO blocking I/O on tokio runtime; functions are sync (called from blocking contexts).
    - example config committed at `config/models_config.toml.example` — never `config/models_config.toml` (operator populates real digests).
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-embedders digest::tests` passes.
    - 16 MB synthetic file (4 chunks aligned) digest matches reference Sha256::digest.
    - 5 MB synthetic file (1 full + 1 partial chunk) matches reference.
    - manifest digest is order-independent for input slice.
    - parse_hex_digest rejects "abc" (3 chars), "g..." (non-hex), 63 chars, 65 chars.
    - models_config.toml.example parses cleanly with current EmbedderRegistration schema.
    - 1-byte tampered file → DigestMismatch; SHAs differ in expected/actual fields.
  </verification>
</definition_of_done>

<pseudo_code>
digest.rs:
  - streaming_sha256: Sha256::new(); loop read up to STREAMING_CHUNK_BYTES; update; return finalize as [u8; 32].
  - streaming_sha256_file: File::open + BufReader::new + streaming_sha256.
  - manifest_sha256: clone paths, sort by file_name, hash each (filename + ":" + hex_digest_of_file_sha), feed concatenated bytes through Sha256.
  - hex_digest: format!("{:0&gt;64x}", BigEndian-encoded); use `hex::encode` if hex crate is in workspace, else manual.
  - parse_hex_digest: validate 64 chars + each is in [0-9a-f]; convert pair-by-pair.
  - load_models_config: read_to_string + toml::from_str.
  - verify_embedder_digest: dispatch single-vs-multi-file; compute actual; compare; return DigestMismatch on mismatch.
  - Tests with tempfile for synthetic fixtures.

lib.rs:
  - pub mod digest;
  - pub use digest::{streaming_sha256_file, manifest_sha256, hex_digest, ModelsConfig, load_models_config, verify_embedder_digest};
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-embedders/src/digest.rs">Streaming SHA-256 + ModelsConfig + verify</file>
  <file path="config/models_config.toml.example">Per-embedder weight pin schema example</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-embedders/src/lib.rs">Add `pub mod digest;` + re-exports</file>
  <file path="crates/context-graph-mejepa-embedders/Cargo.toml">Add `toml` workspace dep if absent</file>
</files_to_modify>

<validation_criteria>
  <criterion>Streaming digest matches sha2 reference on 4 MB-aligned + non-aligned files</criterion>
  <criterion>Manifest digest is sort-order-independent across input permutations</criterion>
  <criterion>parse_hex_digest rejects malformed inputs</criterion>
  <criterion>verify_embedder_digest returns DigestMismatch on tampered byte</criterion>
  <criterion>config/models_config.toml.example parses against ModelsConfig schema</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-mejepa-embedders digest</command>
  <command>cargo clippy -p context-graph-mejepa-embedders --all-features -- -D warnings</command>
</test_commands>

</task_spec>
```
