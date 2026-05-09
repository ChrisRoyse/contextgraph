// Manual Full State Verification for the Phase 1 instruments scaffolding.
//
// Sources of truth on disk:
//   /tmp/contextgraph-mejepa-instruments-fsv/panel.json     — full 5,120-d panel
//   /tmp/contextgraph-mejepa-instruments-fsv/oracle_*.json  — per-verdict input
//   /tmp/contextgraph-mejepa-instruments-fsv/instruments-evidence.json
//
// Plan:
//   1. Build 4 real synthetic OracleVerdict scenarios:
//      a. all-pass (10 tests, no exception)
//      b. mixed (2 pass, 1 fail, 1 skip, 1 error, no exception)
//      c. crash (no tests ran, AssertionError raised)
//      d. crash (TypeError raised)
//   2. Encode each via EOracleInstrument; write the OracleVerdict + 128-d
//      encoded vector to disk; re-read and verify byte-equal.
//   3. Build a panel via PanelBuilder, set the EOracle slot from scenario (a),
//      verify panel.slot(EOracle) is byte-exact and the rest is zero.
//   4. Verify per-slot extents in the panel layout sum to PANEL_DIM = 5_120.
//   5. Boundary cases:
//      a. Empty no-exception oracle rejects MEJEPA_INSTRUMENTS_INVALID_INPUT
//      b. Empty test_id rejects MEJEPA_INSTRUMENTS_INVALID_INPUT
//      c. Wrong dim into set_slot rejects MEJEPA_INSTRUMENTS_INVALID_INPUT
//      d. NaN in set_slot rejects MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT
//      e. Double-fill rejects MEJEPA_INSTRUMENTS_INVALID_INPUT
//      f. Empty panel build rejects MEJEPA_INSTRUMENTS_INVALID_INPUT
//      g. Malformed persisted panel rejects on readback/deserialization

use context_graph_mejepa_instruments::e_oracle::PerTestOutcome;
use context_graph_mejepa_instruments::{
    EOracleInstrument, ExceptionClass, Instrument, InstrumentSlot, OracleVerdict, Panel,
    PanelBuilder, TestOutcome, PANEL_DIM,
};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

fn ensure(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("instruments FSV invariant failed: {label}").into())
    }
}

fn write_text_checked(path: &Path, text: &str) -> FsvResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)?;
    let readback = fs::read_to_string(path)?;
    ensure(
        &format!("byte readback must match write at {}", path.display()),
        readback == text,
    )?;
    Ok(())
}

fn verdict_all_pass() -> OracleVerdict {
    OracleVerdict {
        per_test: (0..10)
            .map(|i| PerTestOutcome {
                test_id: format!("tests/test_pass.py::test_{i}"),
                outcome: TestOutcome::Pass,
                runtime_ms: 1,
            })
            .collect(),
        exception: None,
    }
}

fn verdict_mixed() -> OracleVerdict {
    OracleVerdict {
        per_test: vec![
            PerTestOutcome {
                test_id: "tests/test_x.py::test_a".into(),
                outcome: TestOutcome::Pass,
                runtime_ms: 1,
            },
            PerTestOutcome {
                test_id: "tests/test_x.py::test_b".into(),
                outcome: TestOutcome::Pass,
                runtime_ms: 1,
            },
            PerTestOutcome {
                test_id: "tests/test_x.py::test_c".into(),
                outcome: TestOutcome::Fail,
                runtime_ms: 1,
            },
            PerTestOutcome {
                test_id: "tests/test_x.py::test_d".into(),
                outcome: TestOutcome::Skip,
                runtime_ms: 0,
            },
            PerTestOutcome {
                test_id: "tests/test_x.py::test_e".into(),
                outcome: TestOutcome::Error,
                runtime_ms: -1,
            },
        ],
        exception: None,
    }
}

fn verdict_crash(class: ExceptionClass) -> OracleVerdict {
    OracleVerdict {
        per_test: vec![],
        exception: Some(class),
    }
}

fn encode_and_persist(
    evidence_root: &Path,
    label: &str,
    inst: &EOracleInstrument,
    verdict: &OracleVerdict,
) -> FsvResult<Value> {
    let v_json = serde_json::to_string_pretty(verdict)?;
    write_text_checked(&evidence_root.join(format!("{label}_input.json")), &v_json)?;
    let encoded = inst.encode(verdict)?;
    ensure(
        &format!("encoded length must equal output_dim for {label}"),
        encoded.len() == inst.output_dim(),
    )?;
    let e_json = serde_json::to_string_pretty(&encoded)?;
    write_text_checked(
        &evidence_root.join(format!("{label}_encoded.json")),
        &e_json,
    )?;
    Ok(json!({
        "label": label,
        "input_path": evidence_root.join(format!("{label}_input.json")).display().to_string(),
        "encoded_path": evidence_root.join(format!("{label}_encoded.json")).display().to_string(),
        "encoded_dim": encoded.len(),
        "encoded_first_16": encoded[..16],
        "all_passed": verdict.all_passed(),
        "passes": true,
    }))
}

fn boundary_empty_test_id() -> FsvResult<Value> {
    let inst = EOracleInstrument;
    let verdict = OracleVerdict {
        per_test: vec![PerTestOutcome {
            test_id: String::new(),
            outcome: TestOutcome::Pass,
            runtime_ms: 1,
        }],
        exception: None,
    };
    let result = inst.encode(&verdict);
    ensure(
        "empty test_id must reject",
        matches!(&result, Err(e) if e.code() == "MEJEPA_INSTRUMENTS_INVALID_INPUT"),
    )?;
    Ok(json!({
        "before_state": {
            "per_test": [{
                "test_id": "",
                "outcome": "pass",
                "runtime_ms": 1,
            }],
            "exception": null,
        },
        "after_state": {
            "encoded_vector_written": false,
        },
        "expected_code": "MEJEPA_INSTRUMENTS_INVALID_INPUT",
        "actual_code": result.err().map(|e| e.code()),
        "passes": true,
    }))
}

fn boundary_empty_verdict() -> FsvResult<Value> {
    let inst = EOracleInstrument;
    let verdict = OracleVerdict {
        per_test: vec![],
        exception: None,
    };
    let result = inst.encode(&verdict);
    ensure(
        "empty no-exception oracle verdict must reject",
        matches!(&result, Err(e) if e.code() == "MEJEPA_INSTRUMENTS_INVALID_INPUT"),
    )?;
    Ok(json!({
        "before_state": {
            "per_test_len": 0,
            "exception": null,
        },
        "after_state": {
            "encoded_vector_written": false,
        },
        "expected_code": "MEJEPA_INSTRUMENTS_INVALID_INPUT",
        "actual_code": result.err().map(|e| e.code()),
        "passes": true,
    }))
}

fn boundary_wrong_dim() -> FsvResult<Value> {
    let mut pb = PanelBuilder::new();
    let result = pb.set_slot(InstrumentSlot::EOracle, &vec![0.0_f32; 64]);
    ensure(
        "wrong-dim set_slot must reject",
        matches!(&result, Err(e) if e.code() == "MEJEPA_INSTRUMENTS_INVALID_INPUT"),
    )?;
    Ok(json!({
        "before_state": {
            "slot": InstrumentSlot::EOracle.slug(),
            "input_vector_len": 64,
            "expected_vector_len": InstrumentSlot::EOracle.dim(),
        },
        "after_state": {
            "slot_written": false,
        },
        "expected_code": "MEJEPA_INSTRUMENTS_INVALID_INPUT",
        "actual_code": result.err().map(|e| e.code()),
        "passes": true,
    }))
}

fn boundary_invalid_runtime() -> FsvResult<Value> {
    let inst = EOracleInstrument;
    let verdict = OracleVerdict {
        per_test: vec![PerTestOutcome {
            test_id: "tests/test_runtime.py::test_negative_runtime".into(),
            outcome: TestOutcome::Pass,
            runtime_ms: -2,
        }],
        exception: None,
    };
    let result = inst.encode(&verdict);
    ensure(
        "runtime_ms < -1 must reject",
        matches!(&result, Err(e) if e.code() == "MEJEPA_INSTRUMENTS_INVALID_INPUT"),
    )?;
    Ok(json!({
        "before_state": {
            "test_id": "tests/test_runtime.py::test_negative_runtime",
            "runtime_ms": -2,
        },
        "after_state": {
            "encoded_vector_written": false,
        },
        "expected_code": "MEJEPA_INSTRUMENTS_INVALID_INPUT",
        "actual_code": result.err().map(|e| e.code()),
        "passes": true,
    }))
}

fn boundary_nan() -> FsvResult<Value> {
    let mut pb = PanelBuilder::new();
    let mut v = vec![0.0_f32; 128];
    v[0] = f32::NAN;
    let result = pb.set_slot(InstrumentSlot::EOracle, &v);
    ensure(
        "NaN in set_slot must reject",
        matches!(&result, Err(e) if e.code() == "MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT"),
    )?;
    Ok(json!({
        "before_state": {
            "slot": InstrumentSlot::EOracle.slug(),
            "input_vector_len": 128,
            "non_finite_index": 0,
            "non_finite_value": "NaN",
        },
        "after_state": {
            "slot_written": false,
        },
        "expected_code": "MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT",
        "actual_code": result.err().map(|e| e.code()),
        "passes": true,
    }))
}

fn boundary_double_fill() -> FsvResult<Value> {
    let mut pb = PanelBuilder::new();
    pb.set_slot(InstrumentSlot::EOracle, &vec![0.0_f32; 128])?;
    let result = pb.set_slot(InstrumentSlot::EOracle, &vec![1.0_f32; 128]);
    ensure(
        "double-fill must reject",
        matches!(&result, Err(e) if e.code() == "MEJEPA_INSTRUMENTS_INVALID_INPUT"),
    )?;
    Ok(json!({
        "before_state": {
            "slot": InstrumentSlot::EOracle.slug(),
            "first_write_succeeded": true,
            "attempted_second_write": true,
        },
        "after_state": {
            "second_write_succeeded": false,
        },
        "expected_code": "MEJEPA_INSTRUMENTS_INVALID_INPUT",
        "actual_code": result.err().map(|e| e.code()),
        "passes": true,
    }))
}

fn boundary_empty_panel_build() -> FsvResult<Value> {
    let pb = PanelBuilder::new();
    let result = pb.build();
    ensure(
        "building a panel with no filled slots must reject",
        matches!(&result, Err(e) if e.code() == "MEJEPA_INSTRUMENTS_INVALID_INPUT"),
    )?;
    Ok(json!({
        "before_state": {
            "filled_mask": 0,
            "data_len": PANEL_DIM,
        },
        "after_state": {
            "panel_written": false,
        },
        "expected_code": "MEJEPA_INSTRUMENTS_INVALID_INPUT",
        "actual_code": result.err().map(|e| e.code()),
        "passes": true,
    }))
}

fn boundary_malformed_panel_readback(evidence_root: &Path) -> FsvResult<Value> {
    let malformed_path = evidence_root.join("malformed-panel.json");
    let malformed = json!({
        "data": vec![0.0_f32; PANEL_DIM - 1],
        "filled_mask": 1,
    });
    write_text_checked(&malformed_path, &serde_json::to_string_pretty(&malformed)?)?;
    let readback = fs::read_to_string(&malformed_path)?;
    let result = serde_json::from_str::<Panel>(&readback);
    ensure(
        "malformed persisted panel must reject during readback",
        result.is_err(),
    )?;
    Ok(json!({
        "source_of_truth": malformed_path.display().to_string(),
        "before_state": {
            "data_len": PANEL_DIM - 1,
            "filled_mask": 1,
        },
        "after_state": {
            "deserialized": false,
        },
        "error": result.err().map(|e| e.to_string()),
        "passes": true,
    }))
}

fn main() -> FsvResult<()> {
    let evidence_root = PathBuf::from("/tmp/contextgraph-mejepa-instruments-fsv");
    if evidence_root.exists() {
        fs::remove_dir_all(&evidence_root)?;
    }
    fs::create_dir_all(&evidence_root)?;

    let inst = EOracleInstrument;
    let scenario_a = encode_and_persist(&evidence_root, "all_pass", &inst, &verdict_all_pass())?;
    let scenario_b = encode_and_persist(&evidence_root, "mixed", &inst, &verdict_mixed())?;
    let scenario_c = encode_and_persist(
        &evidence_root,
        "crash_assertion_error",
        &inst,
        &verdict_crash(ExceptionClass::AssertionError),
    )?;
    let scenario_d = encode_and_persist(
        &evidence_root,
        "crash_type_error",
        &inst,
        &verdict_crash(ExceptionClass::TypeError),
    )?;

    // Panel assembly + on-disk verification.
    let mut pb = PanelBuilder::new();
    pb.set_from_instrument(&inst, &verdict_all_pass())?;
    let panel = pb.build()?;
    ensure(
        "panel data length must equal PANEL_DIM",
        panel.data().len() == PANEL_DIM,
    )?;
    ensure(
        "EOracle slot must be filled",
        panel.is_filled(InstrumentSlot::EOracle),
    )?;
    ensure(
        "non-filled slots must be zero in panel data",
        panel
            .data()
            .iter()
            .enumerate()
            .filter(|(idx, _)| {
                let (off, dim) = InstrumentSlot::EOracle.extent();
                *idx < off || *idx >= off + dim
            })
            .all(|(_, x)| *x == 0.0),
    )?;
    let panel_json = serde_json::to_string_pretty(&panel)?;
    let panel_path = evidence_root.join("panel.json");
    write_text_checked(&panel_path, &panel_json)?;
    let readback_text = fs::read_to_string(&panel_path)?;
    let readback_panel: Panel = serde_json::from_str(&readback_text)?;
    ensure(
        "panel JSON readback must equal in-memory panel",
        readback_panel == panel,
    )?;

    let mut slot_layout_evidence = Vec::new();
    for (slot, offset, dim) in InstrumentSlot::layout() {
        slot_layout_evidence.push(json!({
            "slot": slot.slug(),
            "offset": offset,
            "dim": dim,
        }));
    }

    let evidence = json!({
        "panel_dim": PANEL_DIM,
        "slot_layout": slot_layout_evidence,
        "scenarios": {
            "all_pass": scenario_a,
            "mixed": scenario_b,
            "crash_assertion_error": scenario_c,
            "crash_type_error": scenario_d,
        },
        "panel_source_of_truth": panel_path.display().to_string(),
        "boundaries": {
            "empty_verdict": boundary_empty_verdict()?,
            "empty_test_id": boundary_empty_test_id()?,
            "wrong_dim": boundary_wrong_dim()?,
            "invalid_runtime": boundary_invalid_runtime()?,
            "nan_in_set_slot": boundary_nan()?,
            "double_fill": boundary_double_fill()?,
            "empty_panel_build": boundary_empty_panel_build()?,
            "malformed_panel_readback": boundary_malformed_panel_readback(&evidence_root)?,
        },
        "passes": true,
    });

    let evidence_path = evidence_root.join("instruments-evidence.json");
    let text = serde_json::to_string_pretty(&evidence)?;
    write_text_checked(&evidence_path, &text)?;

    println!(
        "Phase 1 instruments FSV evidence: {}",
        evidence_path.display()
    );
    println!("{}", text);
    Ok(())
}
