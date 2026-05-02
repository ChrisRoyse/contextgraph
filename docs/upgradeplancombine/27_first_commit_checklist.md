# 27 — First-Commit Checklist (Literal Steps)

This document is the literal "first PR" runbook. An implementer
following the upgrade plan executes these steps in order. Each step
is mechanical; estimated wall-time per step is given.

## 1. Pre-flight (15 min)

```bash
# 1.1 Verify the v1 release-C test suite passes on the documented host
cd /home/cabdru/contextgraph
cargo test --workspace --all-features 2>&1 | tail -20

# 1.2 Verify the existing local CI gate passes
LOCAL_CI_RUN_ID_PREFIX=20260601T_pre_v2_baseline \
  scripts/5090jepa/local_ci_gate.sh smoke-a 2>&1 | tail -3

# 1.3 Capture v1 baseline numbers for v1<->v2 comparison
cp tmp/mejepa_local_ci_runs/20260601T_pre_v2_baseline_smoke_a/paper_tables/*.csv \
   /tmp/v1_baseline_csvs/
ls -la /tmp/v1_baseline_csvs/

# 1.4 Note the v1 release reference SHA-256
sha256sum reference/reference_release_v1.0.0.json
```

If any of the above fails, **stop**. The v2 upgrade requires a clean
v1 baseline.

## 2. Branch creation (1 min)

```bash
git status                              # should be clean
git checkout main
git pull
git checkout -b feature/v2-foundation   # PR 1's branch
```

## 3. PR 1 — Foundation (3 hours of focused work)

### 3.1 Add the three new CF constants

Edit: `crates/context-graph-storage/src/dynamicjepa/column_families.rs`

```diff
 pub const CF_DJ_VERIFICATION_RUNS: &str = "dj_verification_runs";
 pub const CF_DJ_AUDIT_LOG: &str = "dj_audit_log";
+
+// v2 additions (combined ME-JEPA × meaning compression)
+pub const CF_DJ_PAIRWISE_READINGS: &str        = "dj_pairwise_readings";
+pub const CF_DJ_CONSTELLATIONS: &str           = "dj_constellations";
+pub const CF_DJ_THRESHOLD_CALIBRATIONS: &str   = "dj_threshold_calibrations";

 pub const DJ_CF_NAMES: &[&str] = &[
     // ... existing 25 ...
+    CF_DJ_PAIRWISE_READINGS,
+    CF_DJ_CONSTELLATIONS,
+    CF_DJ_THRESHOLD_CALIBRATIONS,
 ];

-pub const DJ_CF_COUNT: usize = DJ_CF_NAMES.len();
+pub const DJ_CF_COUNT: usize = DJ_CF_NAMES.len();   // = 28 in v2
```

### 3.2 Extend `dj_cf_descriptors`

```diff
 pub fn dj_cf_descriptors(cache: &Cache) -> Vec<ColumnFamilyDescriptor> {
     vec![
         // ... existing 25 descriptors ...
+
+        // v2 additions
+        ColumnFamilyDescriptor::new(
+            CF_DJ_PAIRWISE_READINGS,
+            dj_uuid_point_cf_options(cache),
+        ),
+        ColumnFamilyDescriptor::new(
+            CF_DJ_CONSTELLATIONS,
+            dj_composite_index_cf_options(cache),
+        ),
+        ColumnFamilyDescriptor::new(
+            CF_DJ_THRESHOLD_CALIBRATIONS,
+            dj_composite_index_cf_options(cache),
+        ),
     ]
 }
```

### 3.3 Add three new key-builder functions

Create: `crates/context-graph-storage/src/dynamicjepa/keys.rs` (extend
existing file)

```rust
pub fn pairwise_reading_key(
    event_uuid: &Uuid,
    instrument_j: u32,
    instrument_k: u32,
) -> [u8; 24] {
    let mut key = [0u8; 24];
    key[0..16].copy_from_slice(event_uuid.as_bytes());
    key[16..20].copy_from_slice(&instrument_j.to_le_bytes());
    key[20..24].copy_from_slice(&instrument_k.to_le_bytes());
    key
}

pub fn constellation_key(
    domain_uuid: &Uuid,
    subject_id: &str,
    modality_id: u32,
) -> [u8; 36] {
    let mut key = [0u8; 36];
    key[0..16].copy_from_slice(domain_uuid.as_bytes());
    let hash = Sha256::digest(subject_id.as_bytes());
    key[16..32].copy_from_slice(&hash[..16]);
    key[32..36].copy_from_slice(&modality_id.to_le_bytes());
    key
}

pub fn threshold_calibration_key(
    domain_uuid: &Uuid,
    subject_id: &str,
    modality_id: u32,
    supersede_seq: u32,
) -> [u8; 40] {
    let mut key = [0u8; 40];
    key[0..36].copy_from_slice(&constellation_key(domain_uuid, subject_id, modality_id));
    key[36..40].copy_from_slice(&supersede_seq.to_le_bytes());
    key
}
```

### 3.4 Build & test

```bash
cargo check -p context-graph-storage --all-features
cargo clippy -p context-graph-storage --all-targets --all-features -- -D warnings
cargo test -p context-graph-storage --all-features
```

All should pass. If clippy or tests fail at this step, **stop and
fix before proceeding**.

### 3.5 Add the schema-version pre-check

Edit: `crates/context-graph-cli/src/main.rs` (or
`crates/context-graph-cli/src/commands/mod.rs` depending on
existing layout):

```rust
fn validate_bundle_schema_version(
    bundle_root: &Path,
    expected_schema_version: u32,
) -> Result<(), CliError> {
    let manifest_path = bundle_root.join("run_manifest.json");
    if !manifest_path.exists() {
        return Ok(()); // greenfield bundle; will be created with v2 schema
    }

    let manifest: serde_json::Value = serde_json::from_reader(
        File::open(&manifest_path)
            .map_err(|e| CliError::ManifestReadFailed(e))?,
    )
    .map_err(|e| CliError::ManifestParseFailed(e))?;

    let bundle_schema_version = manifest
        .get("bundle_schema_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(1) as u32;

    if bundle_schema_version != expected_schema_version {
        return Err(CliError::BundleSchemaVersionMismatch {
            expected: expected_schema_version,
            found: bundle_schema_version,
            bundle_root: bundle_root.to_path_buf(),
        });
    }

    Ok(())
}
```

Wire it into `run_dynamicjepa()` before any subcommand dispatch:

```rust
pub fn run_dynamicjepa(args: DynamicJepaArgs) -> Result<(), CliError> {
    // ... pre-existing setup ...

    if let Some(bundle_root) = args.bundle_root.as_ref() {
        validate_bundle_schema_version(bundle_root, 2)?;
    }

    match args.command {
        // ... existing 29 dispatches ...
    }
}
```

### 3.6 Add `BundleSchemaVersionMismatch` error variant

Edit: `crates/context-graph-cli/src/errors.rs`

```rust
pub enum CliError {
    // ... existing variants ...

    #[error("Bundle schema version mismatch: expected v{expected}, found v{found} in {bundle_root}")]
    BundleSchemaVersionMismatch {
        expected: u32,
        found: u32,
        bundle_root: PathBuf,
    },

    #[error("Manifest read failed: {0}")]
    ManifestReadFailed(std::io::Error),

    #[error("Manifest parse failed: {0}")]
    ManifestParseFailed(serde_json::Error),
}
```

### 3.7 Run-manifest writer extension

Edit: the `run_manifest.json` writer (search `crates/context-graph-cli/src/commands/`
for `run_manifest`). Add `bundle_schema_version: 2` to the JSON
output:

```rust
let manifest = serde_json::json!({
    "bundle_schema_version": 2,
    "operation": "...",
    // ... existing fields ...
});
serde_json::to_writer_pretty(file, &manifest)?;
```

### 3.8 Tests (PR 1)

Create: `crates/context-graph-storage/src/dynamicjepa/keys.rs` test
module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_pairwise_reading_key_format_24_bytes_fixed() {
        let event = Uuid::new_v4();
        let key = pairwise_reading_key(&event, 1, 2);
        assert_eq!(key.len(), 24);
        assert_eq!(&key[0..16], event.as_bytes());
        assert_eq!(&key[16..20], &1u32.to_le_bytes());
        assert_eq!(&key[20..24], &2u32.to_le_bytes());
    }

    #[test]
    fn test_constellation_key_format_36_bytes_fixed() {
        let domain = Uuid::new_v4();
        let key = constellation_key(&domain, "global", 5);
        assert_eq!(key.len(), 36);
        assert_eq!(&key[0..16], domain.as_bytes());
        assert_eq!(&key[32..36], &5u32.to_le_bytes());
    }

    #[test]
    fn test_threshold_calibration_key_format_40_bytes_fixed() {
        let domain = Uuid::new_v4();
        let key = threshold_calibration_key(&domain, "global", 5, 0);
        assert_eq!(key.len(), 40);
        assert_eq!(&key[36..40], &0u32.to_le_bytes());
    }

    #[test]
    fn test_subject_id_hash_stable_across_calls() {
        let domain = Uuid::new_v4();
        let key1 = constellation_key(&domain, "global", 1);
        let key2 = constellation_key(&domain, "global", 1);
        assert_eq!(key1, key2);
    }
}
```

Create: `crates/context-graph-cli/tests/v2_v1_isolation_fsv.rs`
(NEW) — the back-compat tests:

```rust
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn cli_binary_path() -> &'static str {
    env!("CARGO_BIN_EXE_context-graph")
}

#[test]
fn v2_binary_refuses_v1_run_manifest_typed_error() {
    let temp = TempDir::new().unwrap();
    let bundle_root = temp.path();

    // Write a v1 manifest (no bundle_schema_version field, or = 1)
    let manifest_path = bundle_root.join("run_manifest.json");
    std::fs::write(&manifest_path, r#"{"operation":"old_v1"}"#).unwrap();

    let output = Command::new(cli_binary_path())
        .args(&[
            "dynamicjepa",
            "inspect-counts",
            "--db", bundle_root.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run cli");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Bundle schema version mismatch") ||
        stderr.contains("BundleSchemaVersionMismatch"),
        "Expected schema-version error, got: {}", stderr
    );
}

#[test]
fn v2_binary_creates_v2_bundle_with_default_schema_version_2() {
    // ...
}

#[test]
fn v2_binary_refuses_mixed_v1_v2_in_aggregate() {
    // ...
}
```

### 3.9 Build, test, and commit

```bash
cargo fmt --all -- --check
cargo check --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features 2>&1 | tail -20

git add crates/context-graph-storage/src/dynamicjepa/column_families.rs
git add crates/context-graph-storage/src/dynamicjepa/keys.rs
git add crates/context-graph-cli/src/errors.rs
git add crates/context-graph-cli/src/main.rs
git add crates/context-graph-cli/tests/v2_v1_isolation_fsv.rs

git status                              # confirm what's staged

git commit -m "$(cat <<'EOF'
feat(storage): add 3 new CFs, schema-version pre-check for v2

Adds three new RocksDB column families to the dynamicjepa registry:
dj_pairwise_readings (24-byte key for pairwise instrument features),
dj_constellations (36-byte key for frozen TCT centroid set), and
dj_threshold_calibrations (40-byte key with supersede sequence).
DJ_CF_COUNT now equals 28 (was 25).

Adds the bundle schema-version pre-check in the CLI dispatcher;
v2 binaries reading a v1 manifest fail with BundleSchemaVersionMismatch
before opening RocksDB, per the no-backwards-compat rule. v1 bundles
remain readable by v1 binaries.

Tests cover the 24/36/40-byte key formats (stable across calls)
and the v1↔v2 isolation behaviour. cargo lints clean; no v1 tests
regress.

Co-Authored-By: Chris Royse <chrisroyseai@gmail.com>
EOF
)"
```

## 4. PR 1 push and merge (5 min)

```bash
# Push and open PR (private repo, manual review)
git push -u origin feature/v2-foundation

# In your fork interface, review:
#   - 5 files changed
#   - cargo lints clean
#   - existing v1 test suite still passes
#   - 3 new key tests pass
#   - 3 new isolation tests pass

# Merge via squash-and-merge or fast-forward; tag the merge commit
git checkout main
git pull
git tag v2.0.0-phase1-pr1
```

## 5. Move to PR 2

```bash
git checkout -b feature/v2-foundation-records main

# Now follow `22_core_records_implementation.md` step-by-step.
```

## 6. The first-day checklist summary

| Step | Wall time | Output |
| --- | ---: | --- |
| Pre-flight | 15 min | clean v1 baseline |
| PR 1 branch + 9 file edits | 2 h | PR 1 ready |
| PR 1 build + test + commit + push | 30 min | PR 1 merged |
| **Day 1 total** | **~3 hours** | Phase 1 PR 1 done |

PR 2 (core records) is the natural next-day work: ~3 more hours.
PR 3 (audit log signal-yield) is similar. After Day 3, Phase 1 is
complete. Phase 2 (pairwise compute-on-write, PR 4) starts on
Day 4 and is the largest single PR — typically 2 days of focused
work.

## 7. Key invariants to preserve at every step

Each commit:

- [x] `cargo fmt --all -- --check` passes
- [x] `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes
- [x] `cargo test --workspace --all-features` passes
- [x] No new "we" / "our" / "us" prose in any new file
- [x] No recruiting / hiring / fairness language in any new docs
- [x] Single-author commit message
- [x] CLAUDE.md no-fallback rule preserved (every new code path is fail-closed)
- [x] CLAUDE.md no-mock-data rule preserved (no test fixtures with hand-fabricated bundle bytes; tests use real RocksDB writes from the v2 binary)
- [x] CLAUDE.md no-backwards-compat rule preserved (no migration shims; v2 binary refuses v1 bundles)

## 8. When to ask for help

Stop and reread the design docs if:

- A typed error doesn't have a documented edge case in the matching design doc.
- A new CF row count doesn't match the formula in `03_signal_yield_math.md`.
- An FSV harness fails on a clean run.
- Cargo lints disagree with the code style of the surrounding v1 code.

The design docs in this directory are the source of truth; if a
discrepancy emerges between this checklist and the design docs, the
design docs win and this checklist gets updated.

## 9. After the upgrade lands

When all 17 PRs are in main and the release-D gate passes:

```bash
# Tag the v2 release candidate
git tag v2.0.0-rc1
git push --tags

# Build the artifact from the passed release-D evidence.
# The local CI gate normally performs this packaging itself.
make release-candidate-archive-v2 RELEASE_ID=mejepa_5090_artifact_v2.0.0_rc1

# Update the Zenodo record (per existing procedure)
# Update CITATION.cff
# Update Zenodo description
# Open the v1.1 paper update PR (PR 16)
```

The complete upgrade — code + tests + paper + release artefact — is
documented across this directory. An implementer following the
plan linearly is expected to ship v2.0 within ~4 weeks of focused
effort.

## 10. Final note on doc-implementation feedback

If during implementation a design doc proves wrong (e.g.\ a
formula in `03_signal_yield_math.md` doesn't match the actual code),
the **design doc is updated first**, the PR is rebased on the new
spec, and the change is recorded in the doc's revision history.
This keeps the docs and code in sync; the docs are not retroactively
edited to match buggy code.
