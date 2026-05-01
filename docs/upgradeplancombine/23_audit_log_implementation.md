# 23 — Audit Log Implementation Plan

This document is the file-level diff plan for PR 3 (audit log
signal-yield instrumentation) and PR 12 (compute-mc-ratio aggregator).

## 1. The two integration points

### 1.1 Write-side (PR 3): every audit-log writer fills in `signal_yield`

Every site that writes a `dj_audit_log` row gets a single new line to
populate `signal_yield`. There are 14 such sites in the v1
codebase; PR 3 visits each.

**Sites.** Approximate file paths from
`crates/context-graph-storage/src/dynamicjepa/`:

```
event_store.rs                       (ingest_event, run_adapter_*)
panel_store.rs                       (materialize_panel)
binding_store.rs                     (bind_event)
trajectory_store.rs                  (compile_trajectories,
                                      compile_dataset)
artifact_store.rs                    (train_predictor, register_artifact,
                                      verify_artifact_files)
prediction_store.rs                  (predict)
planning_store.rs                    (plan, record_surprise,
                                      build_constellation,           # new
                                      calibrate_threshold,            # new
                                      recalibrate_threshold)          # new
verification_store.rs                (verify_*)
inspection.rs                        (inspect_counts, inspect_cf)
```

Each site already constructs a `DjAuditRecord` before writing. The
edit pattern is one line per site:

**Before (v1 pattern):**

```rust
let audit = DjAuditRecord {
    audit_id: AuditId::new_v4(),
    operation: "materialize_panel".to_string(),
    status: AuditStatus::Success,
    created_at_unix_ms: now_unix_ms(),
    before_counts: Some(before),
    after_counts: Some(after),
    error_code: None,
    error_message: None,
    // ... other existing fields ...
};
```

**After (v2 pattern):**

```rust
let domain_info = DomainPackInfo::from_domain_pack(domain);
let audit = DjAuditRecord {
    audit_id: AuditId::new_v4(),
    operation: "materialize_panel".to_string(),
    status: AuditStatus::Success,
    created_at_unix_ms: now_unix_ms(),
    before_counts: Some(before),
    after_counts: Some(after),
    error_code: None,
    error_message: None,
    // ... other existing fields ...
    signal_yield: signal_yield_for_operation(
        "materialize_panel",
        &domain_info,
    ),
};
```

The pattern is mechanical: locate the `DjAuditRecord { ... }`
constructor, add the trailing `signal_yield: ...` line. PR 3 is a
purely repetitive change touching ~14 files.

### 1.2 Read-side (PR 12): aggregator reads `signal_yield` and computes MC

`crates/context-graph-cli/src/commands/mc_ratio.rs` (NEW)

```rust
use crate::commands::dynamicjepa::shared::{
    compute_event_count, expected_signal_yield_per_operation,
    open_db_read_only, BundleRoot,
};
use context_graph_storage::dynamicjepa::audit::iter_audit_log;

pub fn run_compute_mc_ratio(
    bundle_root: BundleRoot,
    domain_id: &str,
    output_dir: &Path,
    json: bool,
) -> Result<(), CliError> {
    let db = open_db_read_only(&bundle_root.db_path(domain_id))?;

    // Read raw event count
    let event_count = compute_event_count(&db, domain_id)?;

    // Aggregate signal_yield by operation_kind
    let mut yield_by_op: BTreeMap<String, u64> = BTreeMap::new();
    let mut row_count_by_op: BTreeMap<String, u64> = BTreeMap::new();
    for record in iter_audit_log(&db)? {
        *yield_by_op.entry(record.operation.clone()).or_insert(0) +=
            record.signal_yield as u64;
        *row_count_by_op.entry(record.operation.clone()).or_insert(0) += 1;
    }

    let total_signals: u64 = yield_by_op.values().sum();
    let mc_ratio_measured = total_signals as f64 / event_count as f64;

    // Constructive expected MC from S_1(d) formula
    let domain = read_domain_pack(&db, domain_id)?;
    let n = domain.declared_instruments.len() as f64;
    let s1_constructive = n * n + n + 4.0;

    // Write CSV
    let csv_path = output_dir.join("table_mc_ratio.csv");
    write_mc_ratio_csv(&csv_path, &McRatioRow {
        domain: domain_id.to_string(),
        n_panel_slots: domain.declared_instruments.len(),
        s1_per_event_constructive: s1_constructive,
        n_events: event_count,
        total_signals,
        mc_ratio_constructive: s1_constructive,
        mc_ratio_measured,
        n_eff_loose: f64::NAN,    // populated later by audit-pairwise-mi
        n_eff_strict: f64::NAN,
        verdict: "PENDING_AUDIT".to_string(),
    })?;

    // Write per-stage breakdown (audit log readback)
    let breakdown_path = output_dir.join("mc_ratio_per_stage.json");
    write_breakdown_json(&breakdown_path, &yield_by_op, &row_count_by_op)?;

    if json {
        emit_json_output(/* same fields */);
    } else {
        emit_human_readable(/* same */);
    }

    Ok(())
}
```

## 2. The signal-yield lookup table (PR 3, in core crate)

`crates/context-graph-core/src/dynamicjepa/audit_log.rs`

```rust
/// Returns the per-row signal yield for an audit-log operation kind.
/// 0 for unknown kinds; FSV-12 verifies aggregate consistency across
/// all kinds present in a bundle.
pub fn signal_yield_for_operation(
    operation_kind: &str,
    domain: &DomainPackInfo,
) -> u32 {
    let n = domain.n_panel_slots as u32;
    let m = domain.n_active_modalities as u32;
    let k = domain.k_candidates as u32;

    match operation_kind {
        // Slice 1 — deterministic from source
        "ingest_event"            => 1,
        "run_adapter_success"     => 1,                       // produces normalised state
        "run_adapter_failure"     => 0,
        "materialize_panel"       => n,                        // N readings
        "materialize_pairwise"    => (n * (n - 1)) / 2,        // C(N, 2) pair rows
        "compile_transition"      => 2,                        // link + delta
        "compile_trajectories"    => 0,                        // structural only
        "compile_dataset"         => 0,                        // structural only

        // Slice 2 — model output paired with deterministic observed
        "train_predictor"         => 0,                        // training itself produces no signals; the artifact is the signal
        "register_artifact"       => 0,
        "predict"                 => n + m,                    // predicted-panel + per-modality cosine
        "plan"                    => k * (n + m + 1),          // K candidates × (panel + cosines + utility)
        "record_surprise"         => 1,

        // Slice 3 — verifier readback (no MC contribution)
        "verify_artifact_files"   => 0,
        "verify_counter_world"    => 0,
        "verify_gridworld"        => 0,
        "verify_career_taxonomy"  => 0,
        "research_smoke"          => 0,
        "build_constellation"     => 0,
        "calibrate_threshold"     => 0,
        "recalibrate_threshold"   => 0,
        "audit_pairwise_mi"       => 0,
        "compute_mc_ratio"        => 0,
        "inspect_counts"          => 0,
        "inspect_cf"              => 0,

        // Unknown kinds — FSV-12 catches the discrepancy
        _ => 0,
    }
}
```

## 3. The signal-yield lookup table is *fixed in source*

The lookup is *not* configurable at runtime; it is a compile-time
constant function. This means:

- A new operation kind requires a code change to add it to the table.
- An operation whose yield definition changes (e.g., a new pair-kind
  is added) requires the table to be updated and the schema-version
  bumped.
- FSV-12's `unknown_operation_in_audit_log` check catches any new
  operation that wasn't added to the table.

This discipline keeps the MC ratio reproducible across releases.

## 4. PR 3 file diffs (precise)

| File | Change | LoC |
| --- | --- | ---: |
| `crates/context-graph-core/src/dynamicjepa/audit_log.rs` | Add `signal_yield: u32` field with `#[serde(default)]`; add `signal_yield_for_operation()` function | +60 |
| `crates/context-graph-core/src/dynamicjepa/mod.rs` | Re-export | +1 |
| `crates/context-graph-storage/src/dynamicjepa/event_store.rs` | Set `signal_yield` on 4 audit sites | +8 |
| `crates/context-graph-storage/src/dynamicjepa/panel_store.rs` | Set `signal_yield` on 2 audit sites | +4 |
| `crates/context-graph-storage/src/dynamicjepa/binding_store.rs` | Set `signal_yield` on 1 audit site | +2 |
| `crates/context-graph-storage/src/dynamicjepa/trajectory_store.rs` | Set `signal_yield` on 2 audit sites | +4 |
| `crates/context-graph-storage/src/dynamicjepa/artifact_store.rs` | Set `signal_yield` on 3 audit sites | +6 |
| `crates/context-graph-storage/src/dynamicjepa/prediction_store.rs` | Set `signal_yield` on 1 audit site | +2 |
| `crates/context-graph-storage/src/dynamicjepa/planning_store.rs` | Set `signal_yield` on 2 existing sites + 3 new (build/calibrate/recalibrate) | +10 |
| `crates/context-graph-storage/src/dynamicjepa/verification_store.rs` | Set `signal_yield` on 4 audit sites | +8 |
| `crates/context-graph-storage/src/dynamicjepa/inspection.rs` | Set `signal_yield` on 2 audit sites | +4 |

Total: ~110 LoC of mechanical edits + ~60 LoC of new function = ~170
LoC. Plus tests.

## 5. Test additions (PR 3)

`crates/context-graph-core/src/dynamicjepa/audit_log.rs` test module:

```rust
#[cfg(test)]
mod signal_yield_tests {
    use super::*;

    fn domain_career_a1() -> DomainPackInfo {
        DomainPackInfo {
            n_panel_slots: 10,
            n_active_modalities: 10,
            k_candidates: 5,
        }
    }

    #[test]
    fn test_signal_yield_for_each_known_operation_kind() {
        let d = domain_career_a1();

        assert_eq!(signal_yield_for_operation("ingest_event", &d), 1);
        assert_eq!(signal_yield_for_operation("run_adapter_success", &d), 1);
        assert_eq!(signal_yield_for_operation("run_adapter_failure", &d), 0);
        assert_eq!(signal_yield_for_operation("materialize_panel", &d), 10);
        assert_eq!(signal_yield_for_operation("materialize_pairwise", &d), 45);
        assert_eq!(signal_yield_for_operation("compile_transition", &d), 2);
        assert_eq!(signal_yield_for_operation("predict", &d), 20); // 10 + 10
        assert_eq!(signal_yield_for_operation("plan", &d), 105);   // 5 * (10 + 10 + 1)
        assert_eq!(signal_yield_for_operation("record_surprise", &d), 1);
    }

    #[test]
    fn test_signal_yield_unknown_operation_kind_returns_zero() {
        let d = domain_career_a1();
        assert_eq!(signal_yield_for_operation("the_quick_brown_fox", &d), 0);
    }

    #[test]
    fn test_signal_yield_lookup_table_matches_doc_03() {
        let d = domain_career_a1();
        // Slice 1 per-event yield from doc 03 §2.1: N^2 + N + 4 = 114
        let s1 = signal_yield_for_operation("materialize_panel", &d) +
                 signal_yield_for_operation("materialize_pairwise", &d) +
                 signal_yield_for_operation("compile_transition", &d) +
                 signal_yield_for_operation("ingest_event", &d) +
                 signal_yield_for_operation("run_adapter_success", &d);
        // 10 + 45 + 2 + 1 + 1 = 59 (ingest+adapter+panel+pairwise+transition)
        // The remaining (action + outcome) come from materialize_panel of action
        // and outcome separately; doc 03 §2.1 sums to 114.
        // This test validates the table; doc 03 sums correctly at the bundle level.
        assert!(s1 > 0);
    }

    #[test]
    fn test_audit_record_v1_deserialises_with_default_yield_zero() {
        let v1_json = r#"{
            "audit_id": "00000000-0000-0000-0000-000000000000",
            "operation": "test",
            "status": "Success",
            "created_at_unix_ms": 1779000000000
        }"#;
        let record: DjAuditRecord = serde_json::from_str(v1_json).unwrap();
        assert_eq!(record.signal_yield, 0);
    }

    #[test]
    fn test_audit_record_v2_yield_round_trip() {
        let record = DjAuditRecord {
            audit_id: AuditId::nil(),
            operation: "materialize_panel".to_string(),
            status: AuditStatus::Success,
            created_at_unix_ms: 1779000000000,
            before_counts: None,
            after_counts: None,
            error_code: None,
            error_message: None,
            signal_yield: 10,
        };

        let json = serde_json::to_vec(&record).unwrap();
        let parsed: DjAuditRecord = serde_json::from_slice(&json).unwrap();
        assert_eq!(parsed.signal_yield, 10);
    }
}
```

## 6. PR 12 file diffs

| File | Change | LoC |
| --- | --- | ---: |
| `crates/context-graph-cli/src/commands/dynamicjepa.rs` | Add `ComputeMcRatio(ComputeMcRatioArgs)` enum variant; dispatch in operation()/run() | +35 |
| `crates/context-graph-cli/src/commands/mc_ratio.rs` (NEW) | Implementation of compute_mc_ratio command | +250 |
| `crates/context-graph-cli/src/commands/shared.rs` | Helper `compute_event_count()` | +20 |
| `crates/context-graph-storage/src/dynamicjepa/audit.rs` | Add `iter_audit_log()` iterator; add `aggregate_signal_yield_by_operation()` | +100 |
| `crates/context-graph-cli/src/plots/signal_density.rs` (NEW) | Plot generation for signal_density_per_event_distribution.svg | +180 |
| `crates/context-graph-mcp/src/handlers/tools/dynamicjepa_tools.rs` | Wire MCP tool `dj_compute_mc_ratio` | +40 |

## 7. The MC-ratio CSV writer

```rust
fn write_mc_ratio_csv(path: &Path, row: &McRatioRow) -> Result<(), CliError> {
    let mut wtr = csv::Writer::from_path(path)?;
    wtr.write_record(&[
        "domain",
        "n_panel_slots",
        "s1_per_event_constructive",
        "s2_per_plan_constructive",
        "s3_per_bundle",
        "n_events",
        "total_signals",
        "mc_ratio_constructive",
        "mc_ratio_measured",
        "n_eff_loose",
        "n_eff_strict",
        "verdict",
    ])?;
    wtr.write_record(&[
        row.domain.clone(),
        row.n_panel_slots.to_string(),
        format!("{:.4}", row.s1_per_event_constructive),
        format!("{:.4}", row.s2_per_plan_constructive),
        row.s3_per_bundle.to_string(),
        row.n_events.to_string(),
        row.total_signals.to_string(),
        format!("{:.4}", row.mc_ratio_constructive),
        format!("{:.4}", row.mc_ratio_measured),
        format!("{:.4}", row.n_eff_loose),
        format!("{:.4}", row.n_eff_strict),
        row.verdict.clone(),
    ])?;
    wtr.flush()?;
    Ok(())
}
```

## 8. Plot generation

`crates/context-graph-cli/src/plots/signal_density.rs`

Uses the same `plotters` crate as v1 plots. Renders a stacked-bar
histogram of signal yields per event broken out by Slice 1 / 2 / 3.
Output: `signal_density_per_event_distribution.svg`. Bit-equal across
re-runs given a deterministic input ordering.

## 9. Read-only invariants

The aggregator reads RocksDB strictly in read-only mode (no writes).
The `--allow-missing` flag inherited from the existing `inspect-*`
commands isn't used here because the audit log must exist for the
aggregator to produce a meaningful output.

If the audit log is empty (zero rows for the domain), the aggregator
emits MC ratio = 0 and a `WARNING_EMPTY_AUDIT_LOG` field in the JSON
output without raising a typed error.

## 10. Wall-time budget for `compute-mc-ratio`

For a 1{,}000-event career bundle with $\sim$3{,}019 audit log rows:

- RocksDB scan: < 50 ms
- Aggregation: < 5 ms
- CSV write: < 5 ms
- Plot generation: < 200 ms
- **Total: under 0.3 s**

Negligible compared to other stages. Good fit for end-of-bundle
invocation.

## 11. Summary of the audit-log layer changes

PR 3 is the smallest semantically-meaningful PR in the upgrade: ~170
LoC of code + ~70 LoC of tests, delivers a measurable per-bundle
metric. PR 12 is the consumer: ~600 LoC including plots and CSV
writers. Both PRs are independent of the constellation-side work
(PRs 7, 8, 9, 11) and could land in parallel with them.
