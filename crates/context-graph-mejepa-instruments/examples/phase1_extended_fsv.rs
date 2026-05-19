// Full State Verification for the Phase 1 concrete instruments.

use context_graph_mejepa_instruments::{
    active_slots, assert_probe_passes, materialize_panel, train_linear_probe, vectors_from_panel,
    CodeInstrumentInput, Diagnostic, DiagnosticSeverity, DiffInstrumentInput, EAstInstrument,
    ECfgInstrument, ECommitMsgInstrument, EDataFlowInstrument, EDiffInstrument, EOracleInstrument,
    EProblemInstrument, EReasoningInstrument, ERuntimeInstrument, EStaticAnalysisInstrument,
    ETestInstrument, ETraceInstrument, ETypeGraphInstrument, EWitnessInstrument, FrozenGuard,
    Instrument, InstrumentSlot, OracleVerdict, Panel, PanelBuilder, PanelEnvelope, PanelKey,
    PanelProvenance, PanelStore, PanelVectorInput, PerTestOutcome, ReasoningEvent, ReasoningInput,
    RuntimeInput, ScalarInput, ScalarsInstrument, StaticAnalysisInput, TestOutcome,
    TextInstrumentInput, TimeStep, TraceEvent, TraceEventKind, TraceInput, WitnessChainInput,
    CANONICAL_WITNESS_FORMAT_VERSION, CF_MEJEPA_PANELS, CF_MEJEPA_PANEL_META, PANEL_DIM,
};
use context_graph_witness::{shake256_32, WitnessEntry, WITNESS_ENTRY_SIZE, ZERO_HASH};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

fn main() -> FsvResult<()> {
    let root = PathBuf::from(
        "/zfs/archive/contextgraph/fsv/contextgraph-mejepa-instruments-fsv/phase1-extended",
    );
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    fs::create_dir_all(&root)?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;

    let code = code_input();
    let diff = diff_input();
    let test_text = text_input("test");
    let problem_text = text_input("problem");
    let commit_text = text_input("commit");
    let trace = trace_input();
    let witness = witness_chain();
    let oracle = verdict_all_pass();
    let static_analysis = static_analysis_input();
    let runtime = runtime_input();
    let reasoning = reasoning_input();
    let scalars = scalar_input();

    let mut panel_builder = PanelBuilder::new();
    panel_builder.set_from_instrument(&EAstInstrument, &code)?;
    panel_builder.set_from_instrument(&ECfgInstrument, &code)?;
    panel_builder.set_from_instrument(&EDataFlowInstrument, &code)?;
    panel_builder.set_from_instrument(&ETypeGraphInstrument, &code)?;
    panel_builder.set_from_instrument(&ETestInstrument, &test_text)?;
    panel_builder.set_from_instrument(&ETraceInstrument, &trace)?;
    panel_builder.set_from_instrument(&EDiffInstrument, &diff)?;
    panel_builder.set_from_instrument(&EWitnessInstrument, &witness)?;
    panel_builder.set_from_instrument(&EOracleInstrument, &oracle)?;
    panel_builder.set_from_instrument(&EProblemInstrument, &problem_text)?;
    panel_builder.set_from_instrument(&ECommitMsgInstrument, &commit_text)?;
    panel_builder.set_from_instrument(&EStaticAnalysisInstrument, &static_analysis)?;
    panel_builder.set_from_instrument(&ERuntimeInstrument, &runtime)?;
    panel_builder.set_from_instrument(&EReasoningInstrument, &reasoning)?;
    panel_builder.set_from_instrument(&ScalarsInstrument, &scalars)?;
    panel_builder.require_slots(&InstrumentSlot::all())?;
    let panel = panel_builder.build()?;

    let panel_path = root.join("panel-all-slots.json");
    write_checked(
        &panel_path,
        serde_json::to_string_pretty(&panel)?.as_bytes(),
    )?;
    let panel_readback: Panel = serde_json::from_str(&fs::read_to_string(&panel_path)?)?;
    require("panel readback equality", panel_readback == panel)?;
    for slot in InstrumentSlot::all() {
        require(
            &format!("{} slot persisted", slot.slug()),
            panel_readback.is_filled(slot),
        )?;
        require(
            &format!("{} slot nonzero", slot.slug()),
            panel_readback.slot(slot).iter().any(|value| *value != 0.0),
        )?;
    }

    let materialized_t2 = materialize_panel(
        TimeStep::T2,
        vectors_from_panel(&panel_readback, &InstrumentSlot::all()),
    )?;
    require(
        "materialized t2 equals direct panel",
        materialized_t2.panel == panel_readback,
    )?;
    let materialized_t0_inputs = active_slots(TimeStep::T0)
        .iter()
        .map(|slot| PanelVectorInput {
            slot: *slot,
            vector: panel_readback.slot(*slot).to_vec(),
        })
        .collect::<Vec<_>>();
    let materialized_t0 = materialize_panel(TimeStep::T0, materialized_t0_inputs)?;
    require(
        "materialized t0 oracle is explicit zero",
        materialized_t0
            .panel
            .slot(InstrumentSlot::EOracle)
            .iter()
            .all(|value| *value == 0.0),
    )?;

    let mut frozen_guard = FrozenGuard::new();
    let frozen_snapshot = frozen_guard.register_panel("target.panel.t2", &panel_readback)?;
    let frozen_verify = frozen_guard.verify_panel("target.panel.t2", &panel_readback)?;
    require(
        "frozen snapshot readback equality",
        frozen_snapshot == frozen_verify,
    )?;

    let manifest_path = PathBuf::from("tasks/swebench-lite/sympy__sympy-20590.json");
    let manifest_bytes = fs::read(&manifest_path)?;
    let provenance = PanelProvenance {
        code_version: format!("{}-{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")),
        embedder_versions: instrument_versions(),
        corpus_sha: sha256_hex_bytes(&manifest_bytes),
        frozen_at_unix_ms: unix_ms_now()?,
        source_sha256: sha256_hex_bytes(code.source.as_bytes()),
    };
    let envelope = PanelEnvelope::try_new(TimeStep::T2, panel_readback.clone(), provenance)?;
    let envelope_path = root.join("panel-envelope-t2.json");
    let envelope_text = serde_json::to_string_pretty(&envelope)?;
    write_checked(&envelope_path, envelope_text.as_bytes())?;
    let envelope_readback: PanelEnvelope =
        serde_json::from_str(&fs::read_to_string(&envelope_path)?)?;
    envelope_readback.validate()?;
    require(
        "panel envelope readback equality",
        envelope_readback == envelope,
    )?;

    let panel_store_path = root.join("panels-rocksdb");
    let panel_store_key = PanelKey::new("sympy__sympy-20590", TimeStep::T2)?;
    {
        let store = PanelStore::open(&panel_store_path)?;
        store.put_envelope(&panel_store_key, &envelope_readback)?;
        store.flush()?;
    }
    let panel_store = PanelStore::open(&panel_store_path)?;
    let stored_envelope = panel_store
        .get_envelope(&panel_store_key)?
        .ok_or_else(|| missing_state("stored panel envelope missing after write"))?;
    let stored_meta = panel_store
        .get_meta(&panel_store_key)?
        .ok_or_else(|| missing_state("stored panel metadata missing after write"))?;
    require(
        "rocksdb panel envelope readback equality",
        stored_envelope == envelope_readback,
    )?;
    require(
        "rocksdb panel meta hash matches envelope",
        stored_meta.panel_hash == envelope_readback.panel_hash,
    )?;
    let rocksdb_panel_count = panel_store.count_cf(CF_MEJEPA_PANELS)?;
    let rocksdb_meta_count = panel_store.count_cf(CF_MEJEPA_PANEL_META)?;
    require("rocksdb panel count == 1", rocksdb_panel_count == 1)?;
    require("rocksdb meta count == 1", rocksdb_meta_count == 1)?;
    let rocksdb_panel_rows = panel_store.scan_cf_json(CF_MEJEPA_PANELS)?;
    let rocksdb_meta_rows = panel_store.scan_cf_json(CF_MEJEPA_PANEL_META)?;

    let probe_report = oracle_probe_report()?;
    assert_probe_passes(&probe_report)?;

    let edge_cases = json!({
        "empty_witness_chain": edge_empty_witness(),
        "tampered_witness_chain": edge_tampered_witness(),
        "blank_static_source": edge_blank_static_source(),
        "diagnostic_line_beyond_source": edge_static_line_oob(),
        "missing_required_slot": edge_missing_required_slot(),
        "code_parse_error": edge_code_parse_error(),
        "unsupported_code_language": edge_unsupported_code_language(),
        "empty_trace": edge_empty_trace(),
        "invalid_runtime_coverage": edge_invalid_runtime_coverage(),
        "blank_reasoning_transcript": edge_blank_reasoning(),
        "invalid_scalar_health": edge_invalid_scalar_health(),
        "frozen_panel_mutation": edge_frozen_panel_mutation(&frozen_guard, &panel_readback),
        "frozen_gradient_violation": edge_frozen_gradient(&frozen_guard),
        "t0_future_slot_rejected": edge_t0_future_slot_rejected(&panel_readback),
        "panel_envelope_hash_mismatch": edge_panel_envelope_hash_mismatch(&envelope),
        "linear_probe_single_class": edge_linear_probe_single_class(),
    });
    require(
        "edge cases all pass",
        edge_cases
            .as_object()
            .expect("edge cases object")
            .values()
            .all(|value| value.get("passes") == Some(&Value::Bool(true))),
    )?;
    let filled_slots: Vec<&'static str> = InstrumentSlot::all()
        .iter()
        .map(InstrumentSlot::slug)
        .collect();

    let evidence = json!({
        "source_of_truth": {
            "root": root.display().to_string(),
            "panel_json": panel_path.display().to_string(),
            "panel_envelope_json": envelope_path.display().to_string(),
            "panel_rocksdb": panel_store_path.display().to_string(),
            "panel_cf": CF_MEJEPA_PANELS,
            "panel_meta_cf": CF_MEJEPA_PANEL_META,
            "swebench_manifest": manifest_path.display().to_string(),
        },
        "happy_path": {
            "before_state": {
                "code_language": code.language.as_str(),
                "code_bytes": code.source.len(),
                "diff_before_bytes": diff.before_source.len(),
                "diff_after_bytes": diff.after_source.len(),
                "test_text_bytes": test_text.text.len(),
                "problem_text_bytes": problem_text.text.len(),
                "commit_text_bytes": commit_text.text.len(),
                "trace_events": trace.events.len(),
                "oracle_tests": oracle.per_test.len(),
                "witness_bytes": witness.chain_bytes.len(),
                "static_diagnostics": static_analysis.diagnostics.len(),
                "runtime_wall_time_ms": runtime.wall_time_ms,
                "reasoning_events": reasoning.events.len(),
                "scalar_files_touched": scalars.files_touched,
                "probe_samples": probe_report.sample_count,
            },
            "after_state": {
                "panel_dim": PANEL_DIM,
                "panel_filled_mask": panel_readback.filled_mask(),
                "filled_slot_count": filled_slots.len(),
                "filled_slots": filled_slots,
                "ast_slot_first_8": &panel_readback.slot(InstrumentSlot::EAst)[0..8],
                "cfg_slot_first_8": &panel_readback.slot(InstrumentSlot::ECfg)[0..8],
                "data_flow_slot_first_8": &panel_readback.slot(InstrumentSlot::EDataFlow)[0..8],
                "type_graph_slot_first_8": &panel_readback.slot(InstrumentSlot::ETypeGraph)[0..8],
                "test_slot_first_8": &panel_readback.slot(InstrumentSlot::ETest)[0..8],
                "trace_slot_first_8": &panel_readback.slot(InstrumentSlot::ETrace)[0..8],
                "diff_slot_first_8": &panel_readback.slot(InstrumentSlot::EDiff)[0..8],
                "oracle_slot_first_8": &panel_readback.slot(InstrumentSlot::EOracle)[0..8],
                "witness_slot_first_8": &panel_readback.slot(InstrumentSlot::EWitness)[0..8],
                "problem_slot_first_8": &panel_readback.slot(InstrumentSlot::EProblem)[0..8],
                "commit_slot_first_8": &panel_readback.slot(InstrumentSlot::ECommitMsg)[0..8],
                "static_slot_first_8": &panel_readback.slot(InstrumentSlot::EStaticAnalysis)[0..8],
                "runtime_slot_first_8": &panel_readback.slot(InstrumentSlot::ERuntime)[0..8],
                "reasoning_slot_first_8": &panel_readback.slot(InstrumentSlot::EReasoning)[0..8],
                "scalars_slot_first_8": &panel_readback.slot(InstrumentSlot::Scalars)[0..8],
                "materialized_t0_mask": materialized_t0.panel.filled_mask(),
                "materialized_t2_mask": materialized_t2.panel.filled_mask(),
                "frozen_snapshot": frozen_snapshot,
                "panel_envelope_hash": envelope_readback.panel_hash,
                "rocksdb_counts": {
                    CF_MEJEPA_PANELS: rocksdb_panel_count,
                    CF_MEJEPA_PANEL_META: rocksdb_meta_count,
                },
                "rocksdb_panel_key": panel_store_key.storage_key(),
                "rocksdb_panel_rows": rocksdb_panel_rows,
                "rocksdb_meta_rows": rocksdb_meta_rows,
                "linear_probe": probe_report,
            },
            "passes": true,
        },
        "edge_cases": edge_cases,
        "passes": true,
    });
    let evidence_path = root.join("phase1-extended-evidence.json");
    let text = serde_json::to_string_pretty(&evidence)?;
    write_checked(&evidence_path, text.as_bytes())?;
    let readback: Value = serde_json::from_str(&fs::read_to_string(&evidence_path)?)?;
    require(
        "evidence JSON readback passes",
        readback.get("passes") == Some(&Value::Bool(true)),
    )?;
    require(
        "evidence JSON readback panel path",
        readback["source_of_truth"]["panel_json"] == panel_path.display().to_string(),
    )?;
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

fn code_input() -> CodeInstrumentInput {
    CodeInstrumentInput {
        language: "python".into(),
        path: "pkg/module.py".into(),
        source: "from math import sqrt\n\nclass Solver:\n    def compute(self, value: int) -> int:\n        if value > 3:\n            return value + 1\n        return 0\n".into(),
    }
}

fn diff_input() -> DiffInstrumentInput {
    DiffInstrumentInput {
        language: "python".into(),
        path: "pkg/module.py".into(),
        before_source: "def compute(value: int) -> int:\n    return value\n".into(),
        after_source: "def compute(value: int) -> int:\n    if value > 3:\n        return value + 1\n    return 0\n".into(),
    }
}

fn text_input(kind: &str) -> TextInstrumentInput {
    TextInstrumentInput {
        text: format!("real SWE-bench {kind} text with assert behavior and expected output"),
        source_id: format!("sympy__sympy-20590::{kind}"),
        language: Some("python".into()),
    }
}

fn trace_input() -> TraceInput {
    TraceInput {
        events: vec![
            TraceEvent {
                function_id: "pkg.module.compute".into(),
                line: 3,
                value_hash: "sha256:1111".into(),
                timestamp_ms: 10,
                kind: TraceEventKind::FunctionEnter,
            },
            TraceEvent {
                function_id: "pkg.module.compute".into(),
                line: 5,
                value_hash: "sha256:2222".into(),
                timestamp_ms: 12,
                kind: TraceEventKind::LineHit,
            },
        ],
    }
}

fn runtime_input() -> RuntimeInput {
    RuntimeInput {
        wall_time_ms: 125,
        peak_rss_bytes: 64 * 1024 * 1024,
        exit_code: 0,
        timed_out: false,
        coverage_percent: Some(83.5),
        network_events: 0,
        filesystem_writes: 12,
    }
}

fn reasoning_input() -> ReasoningInput {
    ReasoningInput {
        task_id: "sympy__sympy-20590".into(),
        transcript: "I inspected the failing test, edited the Symbol slots path, and verified the oracle output.".into(),
        events: vec![ReasoningEvent {
            actor: "claude-code".into(),
            event_type: "edit".into(),
            text: "patched the function that controls slot inheritance".into(),
        }],
    }
}

fn scalar_input() -> ScalarInput {
    ScalarInput {
        bfs_depth: 4,
        blame_age_days: 120,
        churn_lines_30d: 18,
        coverage_delta: 3.5,
        repo_health_score: 0.91,
        files_touched: 2,
        hunks_touched: 3,
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

fn edge_code_parse_error() -> Value {
    let mut input = code_input();
    input.source = "def broken(:\n    return 1\n".into();
    let result = EAstInstrument.encode(&input);
    json_edge(
        "code parse error",
        input.source.len(),
        result.err().map(|e| e.code()),
    )
}

fn edge_unsupported_code_language() -> Value {
    let mut input = code_input();
    input.language = "kotlin".into();
    let result = ECfgInstrument.encode(&input);
    json_edge(
        "unsupported code language",
        input.source.len(),
        result.err().map(|e| e.code()),
    )
}

fn edge_empty_trace() -> Value {
    let result = ETraceInstrument.encode(&TraceInput { events: vec![] });
    json_edge("empty trace", 0, result.err().map(|e| e.code()))
}

fn edge_invalid_runtime_coverage() -> Value {
    let mut input = runtime_input();
    input.coverage_percent = Some(120.0);
    let result = ERuntimeInstrument.encode(&input);
    json_edge(
        "invalid runtime coverage",
        1,
        result.err().map(|e| e.code()),
    )
}

fn edge_blank_reasoning() -> Value {
    let mut input = reasoning_input();
    input.transcript = " \n".into();
    let result = EReasoningInstrument.encode(&input);
    json_edge(
        "blank reasoning transcript",
        1,
        result.err().map(|e| e.code()),
    )
}

fn edge_invalid_scalar_health() -> Value {
    let mut input = scalar_input();
    input.repo_health_score = 1.25;
    let result = ScalarsInstrument.encode(&input);
    json_edge("invalid scalar health", 1, result.err().map(|e| e.code()))
}

fn edge_frozen_panel_mutation(guard: &FrozenGuard, panel: &Panel) -> Value {
    let mut changed = panel.data().to_vec();
    changed[0] += 1.0;
    let result = guard.verify_buffer("target.panel.t2", &changed);
    json_edge_expected(
        "frozen panel mutation",
        changed.len(),
        "MEJEPA_INSTR_FROZEN_VIOLATION",
        result.err().map(|e| e.code()),
    )
}

fn edge_frozen_gradient(guard: &FrozenGuard) -> Value {
    let mut gradient = vec![0.0_f32; PANEL_DIM];
    gradient[17] = 0.5;
    let result = guard.assert_no_gradient("target.panel.t2", &gradient);
    json_edge_expected(
        "frozen gradient violation",
        gradient.len(),
        "MEJEPA_INSTR_FROZEN_VIOLATION",
        result.err().map(|e| e.code()),
    )
}

fn edge_t0_future_slot_rejected(panel: &Panel) -> Value {
    let mut inputs = active_slots(TimeStep::T0)
        .iter()
        .map(|slot| PanelVectorInput {
            slot: *slot,
            vector: panel.slot(*slot).to_vec(),
        })
        .collect::<Vec<_>>();
    inputs.push(PanelVectorInput {
        slot: InstrumentSlot::EOracle,
        vector: panel.slot(InstrumentSlot::EOracle).to_vec(),
    });
    let result = materialize_panel(TimeStep::T0, inputs);
    json_edge(
        "t0 future oracle slot rejected",
        PANEL_DIM,
        result.err().map(|e| e.code()),
    )
}

fn edge_panel_envelope_hash_mismatch(envelope: &PanelEnvelope) -> Value {
    let mut changed = envelope.clone();
    changed.panel_hash = "c".repeat(64);
    let result = changed.validate();
    json_edge(
        "panel envelope hash mismatch",
        PANEL_DIM,
        result.err().map(|e| e.code()),
    )
}

fn edge_linear_probe_single_class() -> Value {
    let features = vec![vec![1.0; 4]; 4];
    let result = train_linear_probe(&features, &[1, 1, 1, 1], 0.25, 11);
    json_edge(
        "linear probe single class",
        4,
        result.err().map(|e| e.code()),
    )
}

fn json_edge(name: &str, before_count: usize, actual_code: Option<&'static str>) -> Value {
    json_edge_expected(
        name,
        before_count,
        "MEJEPA_INSTRUMENTS_INVALID_INPUT",
        actual_code,
    )
}

fn json_edge_expected(
    name: &str,
    before_count: usize,
    expected_code: &'static str,
    actual_code: Option<&'static str>,
) -> Value {
    json!({
        "before_state": { "name": name, "count": before_count },
        "after_state": { "accepted": false },
        "expected_code": expected_code,
        "actual_code": actual_code,
        "passes": actual_code == Some(expected_code),
    })
}

fn oracle_probe_report() -> FsvResult<context_graph_mejepa_instruments::ProbeReport> {
    let mut features = Vec::new();
    let mut labels = Vec::new();
    for i in 0..30 {
        let outcome = if i % 3 == 0 {
            TestOutcome::Fail
        } else {
            TestOutcome::Pass
        };
        let verdict = OracleVerdict {
            per_test: vec![PerTestOutcome {
                test_id: format!("tests/test_phase1.py::test_probe_{i}"),
                outcome,
                runtime_ms: (i + 1) as i64,
            }],
            exception: None,
        };
        features.push(EOracleInstrument.encode(&verdict)?);
        labels.push(if outcome == TestOutcome::Pass { 0 } else { 1 });
    }
    Ok(train_linear_probe(&features, &labels, 0.30, 17)?)
}

fn instrument_versions() -> BTreeMap<String, String> {
    InstrumentSlot::all()
        .iter()
        .map(|slot| (slot.slug().to_string(), "deterministic-v1".to_string()))
        .collect()
}

fn sha256_hex_bytes(bytes: &[u8]) -> String {
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn unix_ms_now() -> FsvResult<i64> {
    let duration = SystemTime::now().duration_since(UNIX_EPOCH)?;
    Ok(i64::try_from(duration.as_millis())?)
}

fn write_checked(path: &Path, bytes: &[u8]) -> FsvResult<()> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    require("byte readback", fs::read(path)? == bytes)?;
    Ok(())
}

fn missing_state(message: &'static str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::NotFound, message)
}

fn require(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("phase1 extended FSV invariant failed: {label}").into())
    }
}
