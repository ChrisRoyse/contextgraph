use super::*;

use context_graph_mejepa_instruments::PanelBuilder;

#[test]
fn from_panel_preserves_slot_hashes_and_missing_slots() {
    let (panel, provenance) = fixture_panel(None);
    let observation = ConstellationObservation::from_panel(
        ConstellationPanelId("panel-a".to_string()),
        "chunk-a".to_string(),
        "cell-python-known-good".to_string(),
        &panel,
        &provenance,
        vec![pair("e_ast", "e_cfg")],
    )
    .unwrap();
    assert_eq!(observation.filled_slot_count(), expected_slot_count());
    assert_eq!(observation.slot_hashes().len(), expected_slot_count());
    assert_eq!(observation.pairwise_relationships.len(), 1);

    let (panel, provenance) = fixture_panel(Some(InstrumentSlot::ETrace));
    let observation = ConstellationObservation::from_panel(
        ConstellationPanelId("panel-b".to_string()),
        "chunk-b".to_string(),
        "cell-python-known-good".to_string(),
        &panel,
        &provenance,
        Vec::new(),
    )
    .unwrap();
    assert_eq!(observation.missing_slot_ids(), vec!["e_trace".to_string()]);
}

#[test]
fn validation_rejects_bad_schema_shapes() {
    let (panel, provenance) = fixture_panel(None);
    let mut observation = ConstellationObservation::from_panel(
        ConstellationPanelId("panel-c".to_string()),
        "chunk-c".to_string(),
        "cell-python-known-good".to_string(),
        &panel,
        &provenance,
        Vec::new(),
    )
    .unwrap();
    observation.slots[0].dim += 1;
    assert_eq!(
        observation.validate().unwrap_err().code(),
        "MEJEPA_INFER_DIM_MISMATCH"
    );

    let (panel, provenance) = fixture_panel(None);
    let mut observation = ConstellationObservation::from_panel(
        ConstellationPanelId("panel-d".to_string()),
        "chunk-d".to_string(),
        "cell-python-known-good".to_string(),
        &panel,
        &provenance,
        Vec::new(),
    )
    .unwrap();
    observation.slots[1].slot_id = observation.slots[0].slot_id.clone();
    assert_eq!(
        observation.validate().unwrap_err().code(),
        "MEJEPA_INFER_INVALID_INPUT"
    );

    let err = serde_json::from_value::<ConstellationObservation>(serde_json::json!({
        "schema_version": 1,
        "panel_id": "panel-json",
        "chunk_id": "chunk-json",
        "calibration_cell": "cell",
        "slots": [],
        "pairwise_relationships": [],
        "extra": true
    }))
    .unwrap_err();
    assert!(err.to_string().contains("unknown field"));
}

#[test]
fn validation_rejects_calibration_cell_drift() {
    let (panel, provenance) = fixture_panel(None);
    let mut observation = ConstellationObservation::from_panel(
        ConstellationPanelId("panel-calibration-drift".to_string()),
        "chunk-calibration-drift".to_string(),
        "cell-python-known-good".to_string(),
        &panel,
        &provenance,
        Vec::new(),
    )
    .unwrap();
    observation.slots[0].calibration_cell = "cell-python-other".to_string();
    assert_eq!(
        observation.validate().unwrap_err().code(),
        "MEJEPA_INFER_INVALID_INPUT"
    );

    let (panel, mut provenance) = fixture_panel(None);
    provenance
        .get_mut(&InstrumentSlot::EAst)
        .unwrap()
        .calibration_cell = "cell-python-other".to_string();
    assert_eq!(
        ConstellationObservation::from_panel(
            ConstellationPanelId("panel-provenance-calibration-drift".to_string()),
            "chunk-provenance-calibration-drift".to_string(),
            "cell-python-known-good".to_string(),
            &panel,
            &provenance,
            Vec::new(),
        )
        .unwrap_err()
        .code(),
        "MEJEPA_INFER_INVALID_INPUT"
    );
}

fn fixture_panel(
    missing: Option<InstrumentSlot>,
) -> (Panel, BTreeMap<InstrumentSlot, SlotProvenance>) {
    let mut builder = PanelBuilder::new();
    let mut provenance = BTreeMap::new();
    for slot in InstrumentSlot::all() {
        if Some(slot) == missing {
            continue;
        }
        let vector: Vec<f32> = (0..slot.dim())
            .map(|idx| ((slot.offset() + idx + 1) as f32) / 10_000.0)
            .collect();
        builder.set_slot(slot, &vector).unwrap();
        provenance.insert(slot, provenance_for(slot));
    }
    (builder.build().unwrap(), provenance)
}

fn provenance_for(slot: InstrumentSlot) -> SlotProvenance {
    SlotProvenance {
        instrument_id: format!("instrument:{}", slot.slug()),
        model_version_hash: format!("model-version-{}", slot.slug()),
        source_evidence_id: format!("source-evidence-{}", slot.slug()),
        calibration_cell: "cell-python-known-good".to_string(),
    }
}

fn pair(left: &str, right: &str) -> PairwiseRelationshipObservation {
    PairwiseRelationshipObservation {
        left_slot_id: left.to_string(),
        right_slot_id: right.to_string(),
        relationship_kind: "synthetic_consensus".to_string(),
        correlation: Some(0.25),
        mutual_information: Some(0.5),
        source_evidence_id: "pairwise-fixture".to_string(),
    }
}
