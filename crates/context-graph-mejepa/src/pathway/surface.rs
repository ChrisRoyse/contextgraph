use super::error::{
    PathwayResult, first_closest_historical_pathway_id, invert_interval, require,
    validate_probability,
};
use super::{
    NORMALIZATION_EPSILON, PATHWAY_SCHEMA_VERSION, PathwayLeaf, PathwayLeafEvidence,
    PathwayLeafKind, PathwayLeafOutcome, PathwayNode, PathwaySurfaceInput, PathwaySurfaceReport,
    PathwayTreeRecord, Q5PathwayEventInput, SurfacedPathwayRecord, pathway_cfs,
};

pub fn surface_pathways(input: PathwaySurfaceInput) -> PathwayResult<PathwaySurfaceReport> {
    input.validate()?;
    let tree_id = format!(
        "tree::{}::{}",
        input.prediction_id_hex,
        &input.candidate_patch_sha256[..16]
    );
    let mut branches = Vec::new();
    let q1_yes = q1_leaf(
        "q1:claim_exists:yes",
        PathwayLeafOutcome::Yes,
        input.q1_claim_exists_probability,
        input.q1_conformal_interval,
    );
    let q1_no = q1_leaf(
        "q1:claim_exists:no",
        PathwayLeafOutcome::No,
        1.0 - input.q1_claim_exists_probability,
        invert_interval(input.q1_conformal_interval),
    );
    branches.push(PathwayBranch {
        leaves: vec![q1_no],
        joint_probability: 1.0 - input.q1_claim_exists_probability,
    });
    let q2_pass = q2_leaf(
        "q2:oracle:pass",
        PathwayLeafOutcome::Pass,
        input.q2_oracle_pass_probability,
        input.q2_conformal_interval,
        PathwayLeafEvidence::unknown(),
    );
    let q2_fail = q2_leaf(
        "q2:oracle:fail",
        PathwayLeafOutcome::Fail,
        1.0 - input.q2_oracle_pass_probability,
        invert_interval(input.q2_conformal_interval),
        input.q2_fail_evidence.clone(),
    );
    branches.push(PathwayBranch {
        leaves: vec![q1_yes.clone(), q2_fail],
        joint_probability: input.q1_claim_exists_probability
            * (1.0 - input.q2_oracle_pass_probability),
    });
    let mut q5_branches = vec![PathwayBranch {
        leaves: vec![q1_yes, q2_pass],
        joint_probability: input.q1_claim_exists_probability * input.q2_oracle_pass_probability,
    }];
    for event in input.q5_events {
        let mut next = Vec::with_capacity(q5_branches.len() * 2);
        for branch in q5_branches {
            let yes = q5_leaf(&event, PathwayLeafOutcome::Yes, event.occurred_probability);
            let no = q5_leaf(
                &event,
                PathwayLeafOutcome::No,
                1.0 - event.occurred_probability,
            );
            next.push(branch.extend(yes, event.occurred_probability));
            next.push(branch.extend(no, 1.0 - event.occurred_probability));
        }
        q5_branches = next;
    }
    branches.extend(q5_branches);
    for branch in &branches {
        for leaf in &branch.leaves {
            leaf.validate()?;
        }
        validate_probability("branch_joint_probability", branch.joint_probability)?;
    }
    branches.sort_by(|left, right| {
        right
            .joint_probability
            .total_cmp(&left.joint_probability)
            .then_with(|| left.signature().cmp(&right.signature()))
    });
    let mut selected = branches
        .iter()
        .filter(|branch| branch.joint_probability >= input.prune_epsilon)
        .cloned()
        .collect::<Vec<_>>();
    if selected.is_empty() {
        selected.push(branches[0].clone());
    }
    selected.truncate(input.top_k);
    let raw_sum = selected
        .iter()
        .map(|branch| branch.joint_probability)
        .sum::<f32>();
    require(raw_sum > 0.0, "top-K pathway probability mass is zero")?;
    let mut nodes = Vec::new();
    let mut surfaced_pathways = Vec::new();
    let mut surfaced_pathway_ids = Vec::new();
    for (rank_idx, branch) in selected.iter().enumerate() {
        let mut parent_node_id = None;
        let mut terminal_node_id = String::new();
        let mut cumulative = 1.0_f32;
        for (depth, leaf) in branch.leaves.iter().enumerate() {
            cumulative *= leaf.predicted_probability;
            let node_id = format!("node::{}::{}::{}", tree_id, rank_idx + 1, depth);
            terminal_node_id = node_id.clone();
            nodes.push(PathwayNode {
                node_id: node_id.clone(),
                parent_node_id: parent_node_id.clone(),
                depth: depth as u32,
                leaf: leaf.clone(),
                cumulative_probability: cumulative,
            });
            parent_node_id = Some(node_id);
        }
        let pathway_id = format!("pathway::{}::{:03}", input.prediction_id_hex, rank_idx + 1);
        surfaced_pathway_ids.push(pathway_id.clone());
        let closest_historical_pathway_id =
            first_closest_historical_pathway_id(&branch.leaves).cloned();
        surfaced_pathways.push(SurfacedPathwayRecord {
            schema_version: PATHWAY_SCHEMA_VERSION,
            pathway_id,
            tree_id: tree_id.clone(),
            prediction_id_hex: input.prediction_id_hex.clone(),
            rank: (rank_idx + 1) as u32,
            raw_joint_probability: branch.joint_probability,
            normalized_probability: branch.joint_probability / raw_sum,
            leaf_chain: branch.leaves.clone(),
            terminal_node_id,
            closest_historical_pathway_id,
            cold_cell_warning: branch.leaves.iter().any(|leaf| leaf.cold_cell_warning),
            unknown_pathway_signature: branch
                .leaves
                .iter()
                .any(|leaf| leaf.evidence.unknown_signature),
            created_at_unix_ms: input.created_at_unix_ms,
        });
    }
    let tree = PathwayTreeRecord {
        schema_version: PATHWAY_SCHEMA_VERSION,
        tree_id,
        prediction_id_hex: input.prediction_id_hex,
        candidate_patch_sha256: input.candidate_patch_sha256,
        nodes,
        surfaced_pathway_ids,
        generated_branch_count: branches.len() as u64,
        ambiguous_leaves_in_pathway: 0,
        created_at_unix_ms: input.created_at_unix_ms,
    };
    tree.validate()?;
    let top_k_probability_sum = surfaced_pathways
        .iter()
        .map(|pathway| pathway.normalized_probability)
        .sum::<f32>();
    require(
        (top_k_probability_sum - 1.0).abs() <= NORMALIZATION_EPSILON,
        "top-K normalized pathway probabilities must sum to 1",
    )?;
    for pathway in &surfaced_pathways {
        pathway.validate()?;
    }
    Ok(PathwaySurfaceReport {
        schema_version: PATHWAY_SCHEMA_VERSION,
        tree,
        surfaced_pathways,
        top_k_probability_sum,
        ambiguous_leaves_in_pathway: 0,
        source_of_truth_cfs: pathway_cfs(),
    })
}

pub fn reject_ambiguous_leaf(leaf: &PathwayLeaf) -> PathwayResult<()> {
    leaf.validate()
}

#[derive(Debug, Clone)]
struct PathwayBranch {
    leaves: Vec<PathwayLeaf>,
    joint_probability: f32,
}

impl PathwayBranch {
    fn extend(&self, leaf: PathwayLeaf, probability: f32) -> Self {
        let mut leaves = self.leaves.clone();
        leaves.push(leaf);
        Self {
            leaves,
            joint_probability: self.joint_probability * probability,
        }
    }

    fn signature(&self) -> String {
        self.leaves
            .iter()
            .map(|leaf| format!("{}:{:?}", leaf.leaf_id, leaf.predicted_outcome))
            .collect::<Vec<_>>()
            .join("|")
    }
}

fn q1_leaf(
    leaf_id: &str,
    predicted_outcome: PathwayLeafOutcome,
    predicted_probability: f32,
    conformal_interval: [f32; 2],
) -> PathwayLeaf {
    PathwayLeaf {
        leaf_id: leaf_id.to_string(),
        leaf_kind: PathwayLeafKind::Q1ClaimExists,
        predicted_outcome,
        predicted_probability,
        conformal_interval,
        event_id: None,
        event_label: Some("claim_exists".to_string()),
        cold_cell_warning: false,
        evidence: PathwayLeafEvidence::unknown(),
    }
}

fn q2_leaf(
    leaf_id: &str,
    predicted_outcome: PathwayLeafOutcome,
    predicted_probability: f32,
    conformal_interval: [f32; 2],
    evidence: PathwayLeafEvidence,
) -> PathwayLeaf {
    PathwayLeaf {
        leaf_id: leaf_id.to_string(),
        leaf_kind: PathwayLeafKind::Q2OraclePass,
        predicted_outcome,
        predicted_probability,
        conformal_interval,
        event_id: None,
        event_label: Some("oracle_pass".to_string()),
        cold_cell_warning: false,
        evidence,
    }
}

fn q5_leaf(
    event: &Q5PathwayEventInput,
    predicted_outcome: PathwayLeafOutcome,
    predicted_probability: f32,
) -> PathwayLeaf {
    PathwayLeaf {
        leaf_id: format!(
            "q5:{}:{}",
            event.event_id,
            match predicted_outcome {
                PathwayLeafOutcome::Yes => "yes",
                PathwayLeafOutcome::No => "no",
                PathwayLeafOutcome::Pass | PathwayLeafOutcome::Fail => "invalid",
            }
        ),
        leaf_kind: PathwayLeafKind::Q5ShiftEvent,
        predicted_outcome,
        predicted_probability,
        conformal_interval: if predicted_outcome == PathwayLeafOutcome::Yes {
            event.conformal_interval
        } else {
            invert_interval(event.conformal_interval)
        },
        event_id: Some(event.event_id.clone()),
        event_label: Some(event.event_label.clone()),
        cold_cell_warning: event.cold_cell_warning,
        evidence: event.evidence.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pathway::PATHWAY_AMBIGUOUS_LEAF_REJECTED;

    #[test]
    fn surface_pathways_rejects_ambiguous_leaf() {
        let leaf = PathwayLeaf {
            leaf_id: "q4:ambiguous".to_string(),
            leaf_kind: PathwayLeafKind::Ambiguous,
            predicted_outcome: PathwayLeafOutcome::Yes,
            predicted_probability: 0.5,
            conformal_interval: [0.4, 0.6],
            event_id: None,
            event_label: Some("ambiguous".to_string()),
            cold_cell_warning: false,
            evidence: PathwayLeafEvidence::unknown(),
        };
        let err = reject_ambiguous_leaf(&leaf).unwrap_err();
        assert!(err.to_string().contains(PATHWAY_AMBIGUOUS_LEAF_REJECTED));
    }

    #[test]
    fn surface_pathways_normalizes_top_k_and_preserves_skills() {
        let report = surface_pathways(fixture_input()).unwrap();
        assert_eq!(report.ambiguous_leaves_in_pathway, 0);
        assert!((report.top_k_probability_sum - 1.0).abs() <= NORMALIZATION_EPSILON);
        assert_eq!(report.surfaced_pathways.len(), 5);
        assert!(
            report
                .surfaced_pathways
                .iter()
                .flat_map(|pathway| pathway.leaf_chain.iter())
                .any(|leaf| leaf
                    .evidence
                    .active_skill_ids
                    .contains(&"skill:api".to_string()))
        );
        assert!(
            report
                .surfaced_pathways
                .iter()
                .any(|pathway| pathway.unknown_pathway_signature)
        );
    }

    fn fixture_input() -> PathwaySurfaceInput {
        let evidence = PathwayLeafEvidence {
            accepted_label_ids: vec!["label:api".to_string()],
            active_skill_ids: vec!["skill:api".to_string()],
            higher_ability_ids: vec!["ability:service_contract".to_string()],
            source_membership_keys: vec!["member:chunk-1".to_string()],
            skill_signature_hash: Some(
                "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
            ),
            closest_historical_pathway_id: Some("historical:pathway:api".to_string()),
            unknown_signature: false,
        };
        PathwaySurfaceInput {
            prediction_id_hex: "abcdef1234567890".to_string(),
            candidate_patch_sha256:
                "abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd".to_string(),
            q1_claim_exists_probability: 0.9,
            q1_conformal_interval: [0.82, 0.95],
            q2_oracle_pass_probability: 0.7,
            q2_conformal_interval: [0.61, 0.77],
            q2_fail_evidence: evidence.clone(),
            q5_events: vec![
                Q5PathwayEventInput {
                    event_id: "event:test_isolation".to_string(),
                    event_label: "test_isolation_holds".to_string(),
                    occurred_probability: 0.8,
                    conformal_interval: [0.72, 0.86],
                    cold_cell_warning: false,
                    evidence: evidence.clone(),
                },
                Q5PathwayEventInput {
                    event_id: "event:admin_save".to_string(),
                    event_label: "admin_save_consumer_ok".to_string(),
                    occurred_probability: 0.6,
                    conformal_interval: [0.50, 0.68],
                    cold_cell_warning: true,
                    evidence,
                },
            ],
            top_k: 5,
            prune_epsilon: 0.01,
            created_at_unix_ms: 1_779_360_000_000,
        }
    }
}
