// Tests for crate::lib (Panel, PanelBuilder, InstrumentSlot) and
// crate::e_oracle (EOracleInstrument). Real synthetic OracleVerdict
// inputs; no mocks.

use super::e_oracle::{EOracleInstrument, ExceptionClass, OracleVerdict, PerTestOutcome, TestOutcome};
use super::*;

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
    pb.set_slot(InstrumentSlot::EOracle, &vec![0.0_f32; 128]).unwrap();
    let err = pb
        .set_slot(InstrumentSlot::EOracle, &vec![1.0_f32; 128])
        .unwrap_err();
    assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
}

#[test]
fn panel_zero_filled_when_no_slots_set() {
    let pb = PanelBuilder::new();
    let panel = pb.build();
    assert_eq!(panel.data.len(), PANEL_DIM);
    assert!(panel.data.iter().all(|x| *x == 0.0));
    assert_eq!(panel.filled_mask, 0);
}

#[test]
fn panel_filled_mask_records_which_slots_were_set() {
    let mut pb = PanelBuilder::new();
    pb.set_slot(InstrumentSlot::EOracle, &vec![0.0_f32; 128]).unwrap();
    pb.set_slot(InstrumentSlot::ETest, &vec![1.0_f32; 384]).unwrap();
    let panel = pb.build();
    assert!(panel.is_filled(InstrumentSlot::EOracle));
    assert!(panel.is_filled(InstrumentSlot::ETest));
    assert!(!panel.is_filled(InstrumentSlot::EAst));
}

#[test]
fn panel_slot_view_returns_correct_slice() {
    let mut pb = PanelBuilder::new();
    let v: Vec<f32> = (0..128).map(|i| i as f32).collect();
    pb.set_slot(InstrumentSlot::EOracle, &v).unwrap();
    let panel = pb.build();
    assert_eq!(panel.slot(InstrumentSlot::EOracle), v.as_slice());
}

#[test]
fn panel_builder_require_slots_errors_on_unfilled() {
    let pb = PanelBuilder::new();
    let err = pb
        .require_slots(&[InstrumentSlot::EOracle])
        .unwrap_err();
    assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
}

#[test]
fn panel_builder_require_slots_passes_when_filled() {
    let mut pb = PanelBuilder::new();
    pb.set_slot(InstrumentSlot::EOracle, &vec![0.0_f32; 128]).unwrap();
    pb.require_slots(&[InstrumentSlot::EOracle]).unwrap();
}

#[test]
fn e_oracle_output_dim_is_128() {
    let inst = EOracleInstrument;
    assert_eq!(inst.output_dim(), 128);
    assert_eq!(inst.slot(), InstrumentSlot::EOracle);
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
    for i in 6..16 {
        assert_eq!(v[i], 0.0);
    }
    // Padding zero.
    for i in 16..128 {
        assert_eq!(v[i], 0.0);
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
    assert!((sum - 1.0).abs() < 1e-6, "outcome distribution must sum to 1");
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
    for i in 6..16 {
        if v[i] == 1.0 {
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
fn e_oracle_no_tests_no_exception_yields_all_zero_first_six_dims() {
    let inst = EOracleInstrument;
    let verdict = OracleVerdict {
        per_test: vec![],
        exception: None,
    };
    let v = inst.encode(&verdict).unwrap();
    for i in 0..6 {
        assert_eq!(v[i], 0.0);
    }
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
fn e_oracle_set_via_panel_builder_round_trips_through_slot_view() {
    let inst = EOracleInstrument;
    let verdict = verdict_all_pass(7);
    let mut pb = PanelBuilder::new();
    pb.set_from_instrument(&inst, &verdict).unwrap();
    let panel = pb.build();
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
    let panel = pb.build();
    let json = serde_json::to_string(&panel).unwrap();
    let back: Panel = serde_json::from_str(&json).unwrap();
    assert_eq!(back, panel);

    let v_json = serde_json::to_string(&verdict).unwrap();
    let v_back: OracleVerdict = serde_json::from_str(&v_json).unwrap();
    assert_eq!(v_back, verdict);
}
