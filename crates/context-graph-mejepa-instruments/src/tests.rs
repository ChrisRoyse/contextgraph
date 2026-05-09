// Tests for crate::lib (Panel, PanelBuilder, InstrumentSlot) and
// crate::e_oracle (EOracleInstrument). Real synthetic OracleVerdict
// inputs; no mocks.

use super::e_code::{
    CodeInstrumentInput, DiffInstrumentInput, EAstInstrument, ECfgInstrument, EDataFlowInstrument,
    EDiffInstrument, ETypeGraphInstrument,
};
use super::e_oracle::{
    EOracleInstrument, ExceptionClass, OracleVerdict, PerTestOutcome, TestOutcome,
};
use super::e_reasoning::{EReasoningInstrument, ReasoningEvent, ReasoningInput};
use super::e_runtime::{ERuntimeInstrument, RuntimeInput};
use super::e_scalars::{ScalarInput, ScalarsInstrument};
use super::e_static_analysis::{
    Diagnostic, DiagnosticSeverity, EStaticAnalysisInstrument, StaticAnalysisInput,
};
use super::e_text::{
    ECommitMsgInstrument, EProblemInstrument, ETestInstrument, TextInstrumentInput,
};
use super::e_trace::{ETraceInstrument, TraceEvent, TraceEventKind, TraceInput};
use super::e_witness::{EWitnessInstrument, WitnessChainInput, CANONICAL_WITNESS_FORMAT_VERSION};
use super::*;
use context_graph_witness::{WitnessEntry, HASH_SIZE, ZERO_HASH};

fn verdict_all_pass(n: usize) -> OracleVerdict {
    OracleVerdict {
        per_test: (0..n)
            .map(|i| PerTestOutcome {
                test_id: format!("tests/test_pass.py::test_{i}"),
                outcome: TestOutcome::Pass,
                runtime_ms: 1,
            })
            .collect(),
        exception: None,
    }
}

fn two_entry_witness_chain() -> Vec<u8> {
    let first = WitnessEntry::new(ZERO_HASH, [1u8; HASH_SIZE], 100, 1);
    let second = WitnessEntry::new(first.chain_hash(), [2u8; HASH_SIZE], 250, 2);
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&first.to_bytes());
    bytes.extend_from_slice(&second.to_bytes());
    bytes
}

#[test]
fn slot_layout_sums_to_panel_dim() {
    let total: usize = InstrumentSlot::layout().iter().map(|(_, _, dim)| dim).sum();
    assert_eq!(total, PANEL_DIM);
}

#[test]
fn slot_layout_offsets_are_contiguous_and_non_overlapping() {
    let mut expected = 0usize;
    for (_slot, offset, dim) in InstrumentSlot::layout() {
        assert_eq!(offset, expected);
        expected += dim;
    }
    assert_eq!(expected, PANEL_DIM);
}

#[test]
fn slot_extents_match_doc_09_section_2_2_and_3_5_5() {
    assert_eq!(InstrumentSlot::EAst.dim(), 384);
    assert_eq!(InstrumentSlot::ECfg.dim(), 256);
    assert_eq!(InstrumentSlot::EDataFlow.dim(), 256);
    assert_eq!(InstrumentSlot::ETypeGraph.dim(), 256);
    assert_eq!(InstrumentSlot::ETest.dim(), 384);
    assert_eq!(InstrumentSlot::ETrace.dim(), 512);
    assert_eq!(InstrumentSlot::EDiff.dim(), 256);
    assert_eq!(InstrumentSlot::EWitness.dim(), 256);
    assert_eq!(InstrumentSlot::EOracle.dim(), 128);
    assert_eq!(InstrumentSlot::EProblem.dim(), 1024);
    assert_eq!(InstrumentSlot::ECommitMsg.dim(), 384);
    assert_eq!(InstrumentSlot::EStaticAnalysis.dim(), 256);
    assert_eq!(InstrumentSlot::ERuntime.dim(), 256);
    assert_eq!(InstrumentSlot::EReasoning.dim(), 384);
    assert_eq!(InstrumentSlot::Scalars.dim(), 128);
}

#[test]
fn slot_slugs_are_unique_lowercase_underscores() {
    let mut seen = std::collections::HashSet::new();
    for slot in InstrumentSlot::all() {
        let s = slot.slug();
        assert!(!s.is_empty());
        assert!(s.chars().all(|c| c.is_ascii_lowercase() || c == '_'));
        assert!(seen.insert(s));
    }
    assert_eq!(seen.len(), 15);
}

#[test]
fn panel_builder_set_slot_validates_dim() {
    let mut pb = PanelBuilder::new();
    let err = pb
        .set_slot(InstrumentSlot::EOracle, &vec![0.0_f32; 64])
        .unwrap_err();
    assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
}

#[test]
fn panel_builder_set_slot_rejects_non_finite() {
    let mut pb = PanelBuilder::new();
    let mut v = vec![0.0_f32; 128];
    v[0] = f32::NAN;
    let err = pb.set_slot(InstrumentSlot::EOracle, &v).unwrap_err();
    assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT");
    let mut v = vec![0.0_f32; 128];
    v[5] = f32::INFINITY;
    let err = pb.set_slot(InstrumentSlot::EOracle, &v).unwrap_err();
    assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT");
}

#[test]
fn panel_builder_set_slot_rejects_double_fill() {
    let mut pb = PanelBuilder::new();
    pb.set_slot(InstrumentSlot::EOracle, &vec![0.0_f32; 128])
        .unwrap();
    let err = pb
        .set_slot(InstrumentSlot::EOracle, &vec![1.0_f32; 128])
        .unwrap_err();
    assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
}

#[test]
fn panel_builder_build_rejects_when_no_slots_set() {
    let pb = PanelBuilder::new();
    let err = pb.build().unwrap_err();
    assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
}

#[test]
fn panel_filled_mask_records_which_slots_were_set() {
    let mut pb = PanelBuilder::new();
    pb.set_slot(InstrumentSlot::EOracle, &vec![0.0_f32; 128])
        .unwrap();
    pb.set_slot(InstrumentSlot::ETest, &vec![1.0_f32; 384])
        .unwrap();
    let panel = pb.build().unwrap();
    assert!(panel.is_filled(InstrumentSlot::EOracle));
    assert!(panel.is_filled(InstrumentSlot::ETest));
    assert!(!panel.is_filled(InstrumentSlot::EAst));
    assert_ne!(panel.filled_mask(), 0);
}

#[test]
fn panel_slot_view_returns_correct_slice() {
    let mut pb = PanelBuilder::new();
    let v: Vec<f32> = (0..128).map(|i| i as f32).collect();
    pb.set_slot(InstrumentSlot::EOracle, &v).unwrap();
    let panel = pb.build().unwrap();
    assert_eq!(panel.slot(InstrumentSlot::EOracle), v.as_slice());
}

#[test]
fn panel_builder_require_slots_errors_on_unfilled() {
    let pb = PanelBuilder::new();
    let err = pb.require_slots(&[InstrumentSlot::EOracle]).unwrap_err();
    assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
}

#[test]
fn panel_builder_require_slots_passes_when_filled() {
    let mut pb = PanelBuilder::new();
    pb.set_slot(InstrumentSlot::EOracle, &vec![0.0_f32; 128])
        .unwrap();
    pb.require_slots(&[InstrumentSlot::EOracle]).unwrap();
}

#[test]
fn panel_builder_require_slots_rejects_empty_required_list() {
    let pb = PanelBuilder::new();
    let err = pb.require_slots(&[]).unwrap_err();
    assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
}

#[test]
fn panel_deserialization_rejects_malformed_source_of_truth() {
    let wrong_dim = serde_json::json!({
        "data": vec![0.0_f32; PANEL_DIM - 1],
        "filled_mask": 1u16,
    });
    let err = serde_json::from_value::<Panel>(wrong_dim).unwrap_err();
    assert!(err.to_string().contains("panel data length"));

    let zero_mask = serde_json::json!({
        "data": vec![0.0_f32; PANEL_DIM],
        "filled_mask": 0u16,
    });
    let err = serde_json::from_value::<Panel>(zero_mask).unwrap_err();
    assert!(err.to_string().contains("no filled instrument slots"));

    let invalid_mask = serde_json::json!({
        "data": vec![0.0_f32; PANEL_DIM],
        "filled_mask": 0x8000u16,
    });
    let err = serde_json::from_value::<Panel>(invalid_mask).unwrap_err();
    assert!(err.to_string().contains("unknown slot bits"));

    // Non-finite values cannot be carried by JSON (the spec rejects NaN /
    // Infinity), so we exercise the validation path through Panel::try_new
    // directly rather than via serde_json::from_value.
    let mut non_finite_values = vec![0.0_f32; PANEL_DIM];
    non_finite_values[17] = f32::INFINITY;
    let err = Panel::try_new(non_finite_values, 1u16).unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("non-finite")
            || err.code() == "MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT"
            || err.code() == "MEJEPA_INSTRUMENTS_INVALID_INPUT",
        "unexpected error for non-finite Panel::try_new: {err:?}"
    );
}

#[test]
fn e_oracle_output_dim_is_128() {
    let inst = EOracleInstrument;
    assert_eq!(inst.output_dim(), 128);
    assert_eq!(inst.slot(), InstrumentSlot::EOracle);
}

#[test]
fn e_witness_encodes_verified_chain() {
    let inst = EWitnessInstrument;
    let input = WitnessChainInput {
        format_version: CANONICAL_WITNESS_FORMAT_VERSION,
        chain_bytes: two_entry_witness_chain(),
    };
    let v = inst.encode(&input).unwrap();
    assert_eq!(v.len(), InstrumentSlot::EWitness.dim());
    assert!(v.iter().all(|x| x.is_finite()));
    assert!(v[0] > 0.0);
    assert!(v[132 + 1] > 0.0);
    assert!(v[132 + 2] > 0.0);
}

#[test]
fn e_witness_rejects_empty_or_tampered_chain() {
    let inst = EWitnessInstrument;
    let empty = WitnessChainInput {
        format_version: CANONICAL_WITNESS_FORMAT_VERSION,
        chain_bytes: vec![],
    };
    let err = inst.encode(&empty).unwrap_err();
    assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");

    let mut tampered = two_entry_witness_chain();
    tampered[context_graph_witness::WITNESS_ENTRY_SIZE] ^= 0x7f;
    let err = inst
        .encode(&WitnessChainInput {
            format_version: CANONICAL_WITNESS_FORMAT_VERSION,
            chain_bytes: tampered,
        })
        .unwrap_err();
    assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
}

#[test]
fn e_witness_rejects_legacy_format_without_conversion() {
    let inst = EWitnessInstrument;
    let err = inst
        .encode(&WitnessChainInput {
            format_version: CANONICAL_WITNESS_FORMAT_VERSION + 1,
            chain_bytes: two_entry_witness_chain(),
        })
        .unwrap_err();
    assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
}

#[test]
fn e_static_analysis_encodes_diagnostics_and_shape() {
    let inst = EStaticAnalysisInstrument;
    let input = StaticAnalysisInput {
        source_text:
            "import math\n\n\ndef compute(x):\n    if x > 1:\n        return x + 1\n    return 0\n"
                .into(),
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
                line: 5,
                column: 12,
            },
        ],
        churn_30d: Some(42),
    };
    let v = inst.encode(&input).unwrap();
    assert_eq!(v.len(), InstrumentSlot::EStaticAnalysis.dim());
    assert!(v.iter().all(|x| x.is_finite()));
    assert!(v[0] > 0.0, "error severity histogram must be populated");
    assert!(v[1] > 0.0, "warning severity histogram must be populated");
    assert!(v[64] > 0.0, "LOC feature must be populated");
    assert!(v[192] > 0.0, "churn feature must be populated");
}

#[test]
fn e_static_analysis_rejects_invalid_source_and_diagnostic_lines() {
    let inst = EStaticAnalysisInstrument;
    let err = inst
        .encode(&StaticAnalysisInput {
            source_text: " \n\t".into(),
            diagnostics: vec![],
            churn_30d: None,
        })
        .unwrap_err();
    assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");

    let err = inst
        .encode(&StaticAnalysisInput {
            source_text: "x = 1\n".into(),
            diagnostics: vec![Diagnostic {
                tool: "ruff".into(),
                severity: DiagnosticSeverity::Error,
                category: "E999".into(),
                line: 99,
                column: 1,
            }],
            churn_30d: None,
        })
        .unwrap_err();
    assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
}

fn sample_code_input() -> CodeInstrumentInput {
    CodeInstrumentInput {
        language: "python".into(),
        path: "pkg/module.py".into(),
        source: "from math import sqrt\n\nclass Solver:\n    def compute(self, value: int) -> int:\n        if value > 3:\n            return value + 1\n        return 0\n".into(),
    }
}

fn sample_diff_input() -> DiffInstrumentInput {
    DiffInstrumentInput {
        language: "python".into(),
        path: "pkg/module.py".into(),
        before_source: "def compute(value: int) -> int:\n    return value\n".into(),
        after_source: "def compute(value: int) -> int:\n    if value > 3:\n        return value + 1\n    return 0\n".into(),
    }
}

fn sample_text_input(kind: &str) -> TextInstrumentInput {
    TextInstrumentInput {
        text: format!("real SWE-bench {kind} text with assert behavior and expected output"),
        source_id: format!("sympy__sympy-20590::{kind}"),
        language: Some("python".into()),
    }
}

fn sample_trace_input() -> TraceInput {
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

fn sample_runtime_input() -> RuntimeInput {
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

fn sample_reasoning_input() -> ReasoningInput {
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

fn sample_scalar_input() -> ScalarInput {
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

fn sample_static_input() -> StaticAnalysisInput {
    StaticAnalysisInput {
        source_text: sample_code_input().source,
        diagnostics: vec![Diagnostic {
            tool: "ruff".into(),
            severity: DiagnosticSeverity::Warning,
            category: "F401".into(),
            line: 1,
            column: 1,
        }],
        churn_30d: Some(18),
    }
}

#[test]
fn remaining_phase1_instruments_encode_real_typed_inputs() {
    let code = sample_code_input();
    let diff = sample_diff_input();
    let cases: Vec<(InstrumentSlot, Vec<f32>)> = vec![
        (InstrumentSlot::EAst, EAstInstrument.encode(&code).unwrap()),
        (InstrumentSlot::ECfg, ECfgInstrument.encode(&code).unwrap()),
        (
            InstrumentSlot::EDataFlow,
            EDataFlowInstrument.encode(&code).unwrap(),
        ),
        (
            InstrumentSlot::ETypeGraph,
            ETypeGraphInstrument.encode(&code).unwrap(),
        ),
        (
            InstrumentSlot::ETest,
            ETestInstrument.encode(&sample_text_input("test")).unwrap(),
        ),
        (
            InstrumentSlot::ETrace,
            ETraceInstrument.encode(&sample_trace_input()).unwrap(),
        ),
        (
            InstrumentSlot::EDiff,
            EDiffInstrument.encode(&diff).unwrap(),
        ),
        (
            InstrumentSlot::EProblem,
            EProblemInstrument
                .encode(&sample_text_input("problem"))
                .unwrap(),
        ),
        (
            InstrumentSlot::ECommitMsg,
            ECommitMsgInstrument
                .encode(&sample_text_input("commit"))
                .unwrap(),
        ),
        (
            InstrumentSlot::ERuntime,
            ERuntimeInstrument.encode(&sample_runtime_input()).unwrap(),
        ),
        (
            InstrumentSlot::EReasoning,
            EReasoningInstrument
                .encode(&sample_reasoning_input())
                .unwrap(),
        ),
        (
            InstrumentSlot::Scalars,
            ScalarsInstrument.encode(&sample_scalar_input()).unwrap(),
        ),
    ];

    for (slot, vector) in cases {
        assert_eq!(vector.len(), slot.dim(), "{slot:?}");
        assert!(vector.iter().all(|v| v.is_finite()), "{slot:?}");
        assert!(vector.iter().any(|v| *v != 0.0), "{slot:?}");
    }
}

#[test]
fn code_instruments_reject_recovered_or_unsupported_ast_sources() {
    let mut invalid = sample_code_input();
    invalid.source = "def broken(:\n    return 1\n".into();
    let err = EAstInstrument.encode(&invalid).unwrap_err();
    assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
    assert!(err.to_string().contains("parse"));

    let mut unsupported = sample_code_input();
    unsupported.language = "kotlin".into();
    let err = ECfgInstrument.encode(&unsupported).unwrap_err();
    assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
}

#[test]
fn code_instruments_accept_real_multilanguage_ast_sources() {
    let sources = [
        CodeInstrumentInput {
            language: "rust".into(),
            path: "src/lib.rs".into(),
            source: "pub fn add(a: i32, b: i32) -> i32 { a + b }\n".into(),
        },
        CodeInstrumentInput {
            language: "javascript".into(),
            path: "src/add.js".into(),
            source: "export function add(a, b) { return a + b; }\n".into(),
        },
        CodeInstrumentInput {
            language: "go".into(),
            path: "calc/add.go".into(),
            source: "package calc\n\nfunc Add(a int, b int) int { return a + b }\n".into(),
        },
    ];
    for input in sources {
        let output = EAstInstrument.encode(&input).unwrap();
        assert_eq!(
            output.len(),
            InstrumentSlot::EAst.dim(),
            "{}",
            input.language
        );
        assert!(
            output.iter().any(|value| *value != 0.0),
            "{}",
            input.language
        );
    }
}

#[test]
fn runtime_trace_reasoning_and_scalars_reject_missing_or_invalid_state() {
    let err = ETraceInstrument
        .encode(&TraceInput { events: vec![] })
        .unwrap_err();
    assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");

    let mut runtime = sample_runtime_input();
    runtime.coverage_percent = Some(120.0);
    let err = ERuntimeInstrument.encode(&runtime).unwrap_err();
    assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");

    let mut reasoning = sample_reasoning_input();
    reasoning.transcript = " \n".into();
    let err = EReasoningInstrument.encode(&reasoning).unwrap_err();
    assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");

    let mut scalars = sample_scalar_input();
    scalars.repo_health_score = 1.25;
    let err = ScalarsInstrument.encode(&scalars).unwrap_err();
    assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
}

#[test]
fn full_phase1_panel_can_require_all_15_slots() {
    let mut builder = PanelBuilder::new();
    let code = sample_code_input();
    builder.set_from_instrument(&EAstInstrument, &code).unwrap();
    builder.set_from_instrument(&ECfgInstrument, &code).unwrap();
    builder
        .set_from_instrument(&EDataFlowInstrument, &code)
        .unwrap();
    builder
        .set_from_instrument(&ETypeGraphInstrument, &code)
        .unwrap();
    builder
        .set_from_instrument(&ETestInstrument, &sample_text_input("test"))
        .unwrap();
    builder
        .set_from_instrument(&ETraceInstrument, &sample_trace_input())
        .unwrap();
    builder
        .set_from_instrument(&EDiffInstrument, &sample_diff_input())
        .unwrap();
    builder
        .set_from_instrument(
            &EWitnessInstrument,
            &WitnessChainInput {
                format_version: CANONICAL_WITNESS_FORMAT_VERSION,
                chain_bytes: two_entry_witness_chain(),
            },
        )
        .unwrap();
    builder
        .set_from_instrument(&EOracleInstrument, &verdict_all_pass(2))
        .unwrap();
    builder
        .set_from_instrument(&EProblemInstrument, &sample_text_input("problem"))
        .unwrap();
    builder
        .set_from_instrument(&ECommitMsgInstrument, &sample_text_input("commit"))
        .unwrap();
    builder
        .set_from_instrument(&EStaticAnalysisInstrument, &sample_static_input())
        .unwrap();
    builder
        .set_from_instrument(&ERuntimeInstrument, &sample_runtime_input())
        .unwrap();
    builder
        .set_from_instrument(&EReasoningInstrument, &sample_reasoning_input())
        .unwrap();
    builder
        .set_from_instrument(&ScalarsInstrument, &sample_scalar_input())
        .unwrap();
    builder.require_slots(&InstrumentSlot::all()).unwrap();
    let panel = builder.build().unwrap();
    assert_eq!(panel.data().len(), PANEL_DIM);
    for slot in InstrumentSlot::all() {
        assert!(panel.is_filled(slot), "slot not filled: {slot:?}");
        assert!(
            panel.slot(slot).iter().any(|value| *value != 0.0),
            "slot all zero: {slot:?}"
        );
    }
}

#[test]
fn e_oracle_all_pass_yields_pass_count_one() {
    let inst = EOracleInstrument;
    let v = inst.encode(&verdict_all_pass(10)).unwrap();
    assert_eq!(v.len(), 128);
    // dim 0 is pass-fraction (10/10 = 1.0).
    assert_eq!(v[0], 1.0);
    assert_eq!(v[1], 0.0); // fail
    assert_eq!(v[2], 0.0); // skip
    assert_eq!(v[3], 0.0); // error
    assert_eq!(v[4], 10.0); // total count
    assert_eq!(v[5], 0.0); // has_exception
                           // No exception → no one-hot bit set.
    for value in v.iter().take(16).skip(6) {
        assert_eq!(*value, 0.0);
    }
    // Padding zero.
    for value in v.iter().take(128).skip(16) {
        assert_eq!(*value, 0.0);
    }
}

#[test]
fn e_oracle_mixed_outcomes_yield_distribution_shaped_counts() {
    let inst = EOracleInstrument;
    let verdict = OracleVerdict {
        per_test: vec![
            PerTestOutcome {
                test_id: "t1".into(),
                outcome: TestOutcome::Pass,
                runtime_ms: 1,
            },
            PerTestOutcome {
                test_id: "t2".into(),
                outcome: TestOutcome::Pass,
                runtime_ms: 1,
            },
            PerTestOutcome {
                test_id: "t3".into(),
                outcome: TestOutcome::Fail,
                runtime_ms: 1,
            },
            PerTestOutcome {
                test_id: "t4".into(),
                outcome: TestOutcome::Skip,
                runtime_ms: 1,
            },
            PerTestOutcome {
                test_id: "t5".into(),
                outcome: TestOutcome::Error,
                runtime_ms: -1,
            },
        ],
        exception: None,
    };
    let v = inst.encode(&verdict).unwrap();
    assert!((v[0] - 0.4).abs() < 1e-6); // 2/5 pass
    assert!((v[1] - 0.2).abs() < 1e-6); // 1/5 fail
    assert!((v[2] - 0.2).abs() < 1e-6); // 1/5 skip
    assert!((v[3] - 0.2).abs() < 1e-6); // 1/5 error
    assert_eq!(v[4], 5.0);
    let sum: f32 = v[0..4].iter().sum();
    assert!(
        (sum - 1.0).abs() < 1e-6,
        "outcome distribution must sum to 1"
    );
}

#[test]
fn e_oracle_exception_sets_one_hot_bit() {
    let inst = EOracleInstrument;
    let verdict = OracleVerdict {
        per_test: vec![],
        exception: Some(ExceptionClass::AssertionError),
    };
    let v = inst.encode(&verdict).unwrap();
    assert_eq!(v[5], 1.0); // has_exception
    let assertion_idx = ExceptionClass::AssertionError.one_hot_index();
    assert_eq!(v[6 + assertion_idx], 1.0);
    // No other one-hot bit set.
    let mut hot_count = 0;
    for value in v.iter().take(16).skip(6) {
        if *value == 1.0 {
            hot_count += 1;
        }
    }
    assert_eq!(hot_count, 1);
}

#[test]
fn e_oracle_each_exception_class_lands_in_distinct_one_hot_bit() {
    let inst = EOracleInstrument;
    for class in ExceptionClass::all() {
        let v = inst
            .encode(&OracleVerdict {
                per_test: vec![],
                exception: Some(class),
            })
            .unwrap();
        let hot_indices: Vec<usize> = (6..16).filter(|i| v[*i] == 1.0).collect();
        assert_eq!(
            hot_indices.len(),
            1,
            "class {class:?} produced {} hot bits, expected 1",
            hot_indices.len()
        );
        assert_eq!(hot_indices[0] - 6, class.one_hot_index());
    }
}

#[test]
fn e_oracle_rejects_no_tests_no_exception() {
    let inst = EOracleInstrument;
    let verdict = OracleVerdict {
        per_test: vec![],
        exception: None,
    };
    let err = inst.encode(&verdict).unwrap_err();
    assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
}

#[test]
fn e_oracle_rejects_empty_test_id() {
    let inst = EOracleInstrument;
    let verdict = OracleVerdict {
        per_test: vec![PerTestOutcome {
            test_id: String::new(),
            outcome: TestOutcome::Pass,
            runtime_ms: 1,
        }],
        exception: None,
    };
    let err = inst.encode(&verdict).unwrap_err();
    assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
}

#[test]
fn e_oracle_rejects_whitespace_or_control_character_test_id() {
    let inst = EOracleInstrument;
    for bad_id in ["   ", "tests/test_x.py::test_a\nnext"] {
        let verdict = OracleVerdict {
            per_test: vec![PerTestOutcome {
                test_id: bad_id.to_string(),
                outcome: TestOutcome::Pass,
                runtime_ms: 1,
            }],
            exception: None,
        };
        let err = inst.encode(&verdict).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
    }
}

#[test]
fn e_oracle_rejects_invalid_runtime_sentinel() {
    let inst = EOracleInstrument;
    let verdict = OracleVerdict {
        per_test: vec![PerTestOutcome {
            test_id: "tests/test_x.py::test_a".into(),
            outcome: TestOutcome::Pass,
            runtime_ms: -2,
        }],
        exception: None,
    };
    let err = inst.encode(&verdict).unwrap_err();
    assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
}

#[test]
fn e_oracle_rejects_schema_drift_on_verdict_json() {
    let drifted = serde_json::json!({
        "per_test": [],
        "exception": "other",
        "unexpected": "schema drift",
    });
    let err = serde_json::from_value::<OracleVerdict>(drifted).unwrap_err();
    assert!(err.to_string().contains("unknown field"));
}

#[test]
fn e_oracle_set_via_panel_builder_round_trips_through_slot_view() {
    let inst = EOracleInstrument;
    let verdict = verdict_all_pass(7);
    let mut pb = PanelBuilder::new();
    pb.set_from_instrument(&inst, &verdict).unwrap();
    let panel = pb.build().unwrap();
    let slot_view = panel.slot(InstrumentSlot::EOracle);
    assert_eq!(slot_view.len(), 128);
    assert_eq!(slot_view[0], 1.0);
    assert_eq!(slot_view[4], 7.0);
}

#[test]
fn all_passed_helper_matches_oracle_verdict_semantics() {
    assert!(verdict_all_pass(3).all_passed());
    let mut v = verdict_all_pass(3);
    v.exception = Some(ExceptionClass::AssertionError);
    assert!(!v.all_passed());
    let mut v = verdict_all_pass(3);
    v.per_test[0].outcome = TestOutcome::Fail;
    assert!(!v.all_passed());
    let v_empty = OracleVerdict {
        per_test: vec![],
        exception: None,
    };
    assert!(!v_empty.all_passed());
}

#[test]
fn json_round_trips_panel_and_verdict() {
    let inst = EOracleInstrument;
    let verdict = verdict_all_pass(2);
    let mut pb = PanelBuilder::new();
    pb.set_from_instrument(&inst, &verdict).unwrap();
    let panel = pb.build().unwrap();
    let json = serde_json::to_string(&panel).unwrap();
    let back: Panel = serde_json::from_str(&json).unwrap();
    assert_eq!(back, panel);

    let v_json = serde_json::to_string(&verdict).unwrap();
    let v_back: OracleVerdict = serde_json::from_str(&v_json).unwrap();
    assert_eq!(v_back, verdict);
}
