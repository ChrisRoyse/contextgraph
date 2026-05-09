# TASK-INFER-009 — Source-SHA drift gate + witness chain replay (Gates 5/6 helpers)

```xml
<task_spec id="TASK-INFER-009" version="1.0">

<metadata>
  <title>SHA + witness gate helpers in `crates/context-graph-mejepa/src/gates.rs`</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>9</sequence>
  <implements>
    <item ref="REQ-INFER-13">Gate 5 WitnessChainBroken: replay segment against persisted chain</item>
    <item ref="REQ-INFER-14">Gate 6 SourceShaDrift: claimed pre/post SHAs match disk; canonical paths only</item>
    <item ref="EC-INFER-02">Path traversal `../../etc/passwd` rejected with SourceShaDrift</item>
    <item ref="EC-INFER-06">Witness segment len % 73 != 0 → WitnessChainBroken</item>
    <item ref="EC-INFER-07">Missing file → SourceShaDrift with structured error (not generic IO)</item>
    <item ref="component:check_source_sha_drift">behavior steps 1-2 in TECH-INFERENCE</item>
    <item ref="component:replay_witness_segment">behavior steps 1-3 in TECH-INFERENCE</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INFER-002</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (gates.rs)</estimated_files>
</metadata>

<context>
Gates 5 and 6 are the cheapest in the verify ladder; they run BEFORE compile() to short-circuit
bogus-input cases. SHA pinning checks every claimed path canonically; path traversal rejected per
SEC-MEJEPA-01. Witness chain replay uses the shipped `context-graph-witness` crate's
`verify_chain_bytes` API. Both helpers return `Option&lt;FailedGate&gt;` so verify() can compose them
into the EscalateToHuman ladder cleanly.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (SEC-MEJEPA-01, SEC-MEJEPA-07; failure_taxonomy)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INFERENCE.md (REQ-INFER-13/14; AC-INFER-01-02; AC-INFER-03-02; EC-INFER-02/06/07)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INFERENCE.md (component_contracts check_source_sha_drift, replay_witness_segment)</file>
  <file purpose="upstream_types">crates/context-graph-mejepa/src/types.rs (PatchBundle, FailedGate, AstDiff)</file>
  <file purpose="upstream_error">crates/context-graph-mejepa/src/error.rs (MejepaInferError variants)</file>
  <file purpose="witness_crate">crates/context-graph-witness/src/lib.rs (verify_chain_bytes, WitnessEntry, WITNESS_ENTRY_SIZE = 73)</file>
</input_context_files>

<prerequisites>
  <check>TASK-INFER-001/002 complete</check>
  <check>`crates/context-graph-witness` shipped with `verify_chain_bytes(segment: &[u8], anchor_offset: u64, anchor_hash: [u8; 32]) -> Result<(), WitnessError>` (check upstream API; exact name may differ — task expects an integrity-replay function)</check>
  <check>`sha2 = "0.10"` workspace dep available</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/gates.rs` with:
      - `pub fn check_source_sha_drift(patch: &PatchBundle, repo_root: &std::path::Path) -> Result<Option<FailedGate>, MejepaInferError>` — canonicalize path against repo_root; reject `..` traversal with SourceShaDrift; read bytes; SHA-256; compare to `patch.ast_diff.hunks[i].pre_sha` (and `post_sha` for post-patch state); on mismatch return `Some(FailedGate::SourceShaDrift { path })`; missing file → SourceShaDrift with structured error
      - `pub fn replay_witness_segment(segment: &[u8]) -> Result<Option<FailedGate>, MejepaInferError>` — len % 73 == 0 check (else WitnessChainBroken with offset=0); call `context_graph_witness::verify_chain_bytes(segment, ...)`; on integrity break return `Some(FailedGate::WitnessChainBroken)`; success → None
      - Helper `pub fn canonicalize_under_root(path: &Path, root: &Path) -> Result<PathBuf, MejepaInferError>` — uses `std::fs::canonicalize` + `starts_with(root)` check
    - Add unit tests using `tempfile`:
      - `check_source_sha_drift_valid_paths_returns_none`: synthesize tempdir + file + matching SHA → None
      - `check_source_sha_drift_content_drift_returns_failed_gate`: file bytes changed → SourceShaDrift
      - `check_source_sha_drift_missing_file_returns_failed_gate`: claimed path doesn't exist → SourceShaDrift (NOT generic IO error per EC-INFER-07)
      - `check_source_sha_drift_path_traversal_rejected`: `../../etc/passwd` claimed → SourceShaDrift (SEC-MEJEPA-01)
      - `check_source_sha_drift_symlink_outside_root_rejected`: symlink → outside repo → SourceShaDrift
      - `replay_witness_segment_clean_returns_none`: synthesize a 2-entry valid segment via `WitnessChainBuilder` → None
      - `replay_witness_segment_corrupted_byte_returns_broken`: flip 1 byte → WitnessChainBroken
      - `replay_witness_segment_invalid_length_returns_broken`: len=72 (not multiple of 73) → WitnessChainBroken
      - `canonicalize_under_root_blocks_dotdot`: `tempdir/../etc` → Err
  </in_scope>
  <out_of_scope>
    - Verify ladder ordering — TASK-INFER-004
    - Hooking these helpers into compile() — they're only used by verify()
    - Witness chain segment GENERATION — that's the upstream witness crate's responsibility
    - SourceShaDrift Gate 6 ordering before Gate 5 — TASK-INFER-004
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/gates.rs">
use std::path::{Path, PathBuf};
use crate::types::{PatchBundle, FailedGate};
use crate::error::MejepaInferError;

pub fn check_source_sha_drift(patch: &amp;PatchBundle, repo_root: &amp;Path)
    -&gt; Result&lt;Option&lt;FailedGate&gt;, MejepaInferError&gt;;

pub fn replay_witness_segment(segment: &amp;[u8])
    -&gt; Result&lt;Option&lt;FailedGate&gt;, MejepaInferError&gt;;

pub fn canonicalize_under_root(path: &amp;Path, root: &amp;Path)
    -&gt; Result&lt;PathBuf, MejepaInferError&gt;;

pub fn sha256_of_file(path: &amp;Path) -&gt; Result&lt;[u8; 32], MejepaInferError&gt;;
    </signature>
  </signatures>
  <constraints>
    - Path canonicalization: `std::fs::canonicalize(path)?` then assert `canonical.starts_with(root_canonical)` else SourceShaDrift
    - Reject any control char (0x00-0x1F or 0x7F) in raw path bytes per security rule (BEFORE canonicalize)
    - Missing file → return `SourceShaDrift { path }` (structured), NOT std::io::Error propagated
    - SHA-256 via `sha2::Sha256::digest`
    - WITNESS_ENTRY_SIZE = 73 (from constitution + context-graph-witness)
    - replay_witness_segment.len() max 73 * 8192 = 598_016 bytes per PatchBundle constraint (validated upstream by TASK-INFER-001 try_new)
    - All `?` sites wrap `e.log_context(file!())` per &lt;error_handling&gt;
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa` succeeds
    - `cargo test -p context-graph-mejepa gates` shows ≥ 9 passing tests
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - All tempfile-based tests pass without leaking temp files
    - Path traversal test rejects `../../etc/passwd` clearly
    - Witness segment 1-byte corruption test detects break
  </verification>
</definition_of_done>

<pseudo_code>
use sha2::{Digest, Sha256};

pub fn sha256_of_file(path: &Path) -> Result<[u8; 32], MejepaInferError> {
    let bytes = std::fs::read(path).map_err(|_| MejepaInferError::SourceShaDrift {
        path: path.to_path_buf(),
        claimed: [0u8; 32], observed: [0u8; 32],
    })?;
    let mut h = Sha256::new();
    h.update(&bytes);
    Ok(h.finalize().into())
}

pub fn canonicalize_under_root(path: &Path, root: &Path) -> Result<PathBuf, MejepaInferError> {
    let raw = path.to_string_lossy();
    if raw.bytes().any(|b| b < 0x20 || b == 0x7F) {
        return Err(MejepaInferError::SourceShaDrift {
            path: path.to_path_buf(), claimed: [0u8;32], observed: [0u8;32],
        });
    }
    let root_c = root.canonicalize().map_err(|_| MejepaInferError::SourceShaDrift {
        path: root.to_path_buf(), claimed: [0u8;32], observed: [0u8;32],
    })?;
    let canonical = path.canonicalize().map_err(|_| MejepaInferError::SourceShaDrift {
        path: path.to_path_buf(), claimed: [0u8;32], observed: [0u8;32],
    })?;
    if !canonical.starts_with(&root_c) {
        return Err(MejepaInferError::SourceShaDrift {
            path: canonical, claimed: [0u8;32], observed: [0u8;32],
        });
    }
    Ok(canonical)
}

pub fn check_source_sha_drift(patch, root) -> Result<Option<FailedGate>, MejepaInferError> {
    for hunk in &patch.ast_diff.hunks {
        let canonical = match canonicalize_under_root(&hunk.path, root) {
            Ok(p) => p,
            Err(MejepaInferError::SourceShaDrift { path, .. }) => {
                return Ok(Some(FailedGate::SourceShaDrift { path }));
            },
            Err(e) => return Err(e),
        };
        let observed = sha256_of_file(&canonical)?;
        if observed != hunk.post_sha {
            return Ok(Some(FailedGate::SourceShaDrift { path: canonical }));
        }
    }
    Ok(None)
}

pub fn replay_witness_segment(segment: &[u8]) -> Result<Option<FailedGate>, MejepaInferError> {
    const WITNESS_ENTRY_SIZE: usize = 73;
    if segment.len() % WITNESS_ENTRY_SIZE != 0 {
        return Ok(Some(FailedGate::WitnessChainBroken));
    }
    match context_graph_witness::verify_chain_bytes(segment) {
        Ok(()) => Ok(None),
        Err(_) => Ok(Some(FailedGate::WitnessChainBroken)),
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/gates.rs">check_source_sha_drift + replay_witness_segment + canonicalize_under_root + sha256_of_file + 9 tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/lib.rs">add `pub mod gates;`</file>
  <file path="crates/context-graph-mejepa/Cargo.toml">add `sha2`, `tempfile` (dev-dep) deps; ensure `context-graph-witness` is in [dependencies]</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa gates shows ≥ 9 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>Path traversal `../etc/passwd` rejected with SourceShaDrift</criterion>
  <criterion>Missing file rejected with structured SourceShaDrift (not generic IO)</criterion>
  <criterion>1-byte witness corruption detected</criterion>
  <criterion>Length not multiple of 73 → WitnessChainBroken</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa gates</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
