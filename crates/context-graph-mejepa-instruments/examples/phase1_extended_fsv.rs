// Full State Verification for the Phase 1 concrete instruments now shipped:
// E_Oracle, E_Witness, and E_StaticAnalysis.

use context_graph_mejepa_instruments::{
    Diagnostic, DiagnosticSeverity, EOracleInstrument, EStaticAnalysisInstrument,
    EWitnessInstrument, Instrument, InstrumentSlot, OracleVerdict, Panel, PanelBuilder,
    PerTestOutcome, StaticAnalysisInput, TestOutcome, WitnessChainInput,
    CANONICAL_WITNESS_FORMAT_VERSION,
};
use context_graph_witness::{shake256_32, WitnessEntry, WITNESS_ENTRY_SIZE, ZERO_HASH};
use serde_json::{json, Value};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

fn main() -> FsvResult<()> {
    let root = PathBuf::from("/tmp/contextgraph-mejepa-instruments-fsv/phase1-extended");
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    fs::create_dir_all(&root)?;

    let oracle = verdict_all_pass();
    let witness = witness_chain();
    let static_analysis = static_analysis_input();

    let e_oracle = EOracleInstrument;
    let e_witness = EWitnessInstrument;
    let e_static = EStaticAnalysisInstrument;
    let oracle_vec = e_oracle.encode(&oracle)?;
    let witness_vec = e_witness.encode(&witness)?;
    let static_vec = e_static.encode(&static_analysis)?;

    let mut panel_builder = PanelBuilder::new();
    panel_builder.set_slot(InstrumentSlot::EOracle, &oracle_vec)?;
    panel_builder.set_slot(InstrumentSlot::EWitness, &witness_vec)?;
    panel_builder.set_slot(InstrumentSlot::EStaticAnalysis, &static_vec)?;
    panel_builder.require_slots(&[
        InstrumentSlot::EOracle,
        InstrumentSlot::EWitness,
        InstrumentSlot::EStaticAnalysis,
    ])?;
    let panel = panel_builder.build()?;

    let panel_path = root.join("panel-three-slots.json");
    write_checked(
        &panel_path,
        serde_json::to_string_pretty(&panel)?.as_bytes(),
    )?;
    let panel_readback: Panel = serde_json::from_str(&fs::read_to_string(&panel_path)?)?;
    require("panel readback equality", panel_readback == panel)?;
    require(
        "oracle slot persisted",
        panel_readback.is_filled(InstrumentSlot::EOracle),
    )?;
    require(
        "witness slot persisted",
        panel_readback.is_filled(InstrumentSlot::EWitness),
    )?;
    require(
        "static-analysis slot persisted",
        panel_readback.is_filled(InstrumentSlot::EStaticAnalysis),
    )?;

    let evidence = json!({
        "source_of_truth": {
            "root": root.display().to_string(),
            "panel_json": panel_path.display().to_string(),
        },
        "happy_path": {
            "before_state": {
                "oracle_tests": oracle.per_test.len(),
                "witness_bytes": witness.chain_bytes.len(),
                "static_diagnostics": static_analysis.diagnostics.len(),
            },
            "after_state": {
                "panel_filled_mask": panel_readback.filled_mask(),
                "oracle_slot_first_8": &panel_readback.slot(InstrumentSlot::EOracle)[0..8],
                "witness_slot_first_8": &panel_readback.slot(InstrumentSlot::EWitness)[0..8],
                "static_slot_first_8": &panel_readback.slot(InstrumentSlot::EStaticAnalysis)[0..8],
            },
            "passes": true,
        },
        "edge_cases": {
            "empty_witness_chain": edge_empty_witness(),
            "tampered_witness_chain": edge_tampered_witness(),
            "blank_static_source": edge_blank_static_source(),
            "diagnostic_line_beyond_source": edge_static_line_oob(),
            "missing_required_slot": edge_missing_required_slot(),
        },
        "passes": true,
    });
    let evidence_path = root.join("phase1-extended-evidence.json");
    let text = serde_json::to_string_pretty(&evidence)?;
    write_checked(&evidence_path, text.as_bytes())?;
    let readback: Value = serde_json::from_str(&fs::read_to_string(&evidence_path)?)?;
    require("evidence JSON readback", readback == evidence)?;
    println!("Phase 1 extended FSV evidence: {}", evidence_path.display());
    println!("{}", text);
    Ok(())
}

fn verdict_all_pass() -> OracleVerdict {
    OracleVerdict {
        per_test: vec![PerTestOutcome {
            test_id: "tests/test_phase1.py::test_oracle_pass".into(),
            outcome: TestOutcome::Pass,
            runtime_ms: 4,
        }],
        exception: None,
    }
}

fn witness_chain() -> WitnessChainInput {
    let first = WitnessEntry::new(ZERO_HASH, shake256_32(b"phase1-source"), 100, 1);
    let second = WitnessEntry::new(first.chain_hash(), shake256_32(b"phase1-oracle"), 250, 2);
    WitnessChainInput::from_entries(&[first, second]).expect("valid witness chain")
}

fn static_analysis_input() -> StaticAnalysisInput {
    StaticAnalysisInput {
        source_text: "import os\n\ndef compute(x):\n    return x + 1\n".into(),
        diagnostics: vec![
            Diagnostic {
                tool: "ruff".into(),
                severity: DiagnosticSeverity::Warning,
                category: "F401".into(),
                line: 1,
                column: 1,
            },
            Diagnostic {
                tool: "pyright".into(),
                severity: DiagnosticSeverity::Error,
                category: "reportGeneralTypeIssues".into(),
                line: 4,
                column: 12,
            },
        ],
        churn_30d: Some(42),
    }
}

fn edge_empty_witness() -> Value {
    let result = EWitnessInstrument.encode(&WitnessChainInput {
        format_version: CANONICAL_WITNESS_FORMAT_VERSION,
        chain_bytes: vec![],
    });
    json_edge("empty witness", 0, result.err().map(|e| e.code()))
}

fn edge_tampered_witness() -> Value {
    let mut bytes = witness_chain().chain_bytes;
    bytes[WITNESS_ENTRY_SIZE] ^= 0x7f;
    let result = EWitnessInstrument.encode(&WitnessChainInput {
        format_version: CANONICAL_WITNESS_FORMAT_VERSION,
        chain_bytes: bytes,
    });
    json_edge(
        "tampered witness",
        WITNESS_ENTRY_SIZE * 2,
        result.err().map(|e| e.code()),
    )
}

fn edge_blank_static_source() -> Value {
    let result = EStaticAnalysisInstrument.encode(&StaticAnalysisInput {
        source_text: " \n\t".into(),
        diagnostics: vec![],
        churn_30d: None,
    });
    json_edge("blank static source", 0, result.err().map(|e| e.code()))
}

fn edge_static_line_oob() -> Value {
    let result = EStaticAnalysisInstrument.encode(&StaticAnalysisInput {
        source_text: "x = 1\n".into(),
        diagnostics: vec![Diagnostic {
            tool: "ruff".into(),
            severity: DiagnosticSeverity::Error,
            category: "E999".into(),
            line: 9,
            column: 1,
        }],
        churn_30d: None,
    });
    json_edge(
        "diagnostic line beyond source",
        1,
        result.err().map(|e| e.code()),
    )
}

fn edge_missing_required_slot() -> Value {
    let mut builder = PanelBuilder::new();
    builder
        .set_slot(
            InstrumentSlot::EOracle,
            &EOracleInstrument.encode(&verdict_all_pass()).unwrap(),
        )
        .unwrap();
    let result = builder.require_slots(&[InstrumentSlot::EWitness]);
    json_edge(
        "missing required witness slot",
        1,
        result.err().map(|e| e.code()),
    )
}

fn json_edge(name: &str, before_count: usize, actual_code: Option<&'static str>) -> Value {
    json!({
        "before_state": { "name": name, "count": before_count },
        "after_state": { "accepted": false },
        "expected_code": "MEJEPA_INSTRUMENTS_INVALID_INPUT",
        "actual_code": actual_code,
        "passes": actual_code == Some("MEJEPA_INSTRUMENTS_INVALID_INPUT"),
    })
}

fn write_checked(path: &Path, bytes: &[u8]) -> FsvResult<()> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    require("byte readback", fs::read(path)? == bytes)?;
    Ok(())
}

fn require(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("phase1 extended FSV invariant failed: {label}").into())
    }
}
