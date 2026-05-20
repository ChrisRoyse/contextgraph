mod error;
mod store;
mod surface;
mod types;

use context_graph_mejepa_cf::{
    CF_MEJEPA_OPERATOR_PATHWAY_CHOICES, CF_MEJEPA_PATHWAY_TREES, CF_MEJEPA_SURFACED_PATHWAYS,
};

pub use error::{PathwayError, PathwayResult};
pub use store::{
    persist_pathway_surface, read_operator_pathway_choices, read_pathway_tree,
    read_surfaced_pathway, read_surfaced_pathways_for_prediction, write_operator_pathway_choice,
    write_pathway_tree, write_surfaced_pathway,
};
pub use surface::{pathway_leaf_credit_assignment, reject_ambiguous_leaf, surface_pathways};
pub use types::{
    OperatorPathwayChoiceRecord, PathwayLeaf, PathwayLeafCalibrationReference,
    PathwayLeafCreditAssignment, PathwayLeafEvidence, PathwayLeafKind, PathwayLeafOutcome,
    PathwayNode, PathwaySurfaceInput, PathwaySurfaceReport, PathwayTreeRecord, Q5PathwayEventInput,
    SurfacedPathwayRecord,
};

pub const PATHWAY_SCHEMA_VERSION: u32 = 1;
pub const PATHWAY_AMBIGUOUS_LEAF_REJECTED: &str = "PATHWAY_AMBIGUOUS_LEAF_REJECTED";
pub const UNKNOWN_PATHWAY_SIGNATURE: &str = "UNKNOWN_PATHWAY_SIGNATURE";

pub(crate) const MAX_TOP_K: usize = 20;
pub(crate) const MAX_Q5_EVENTS: usize = 8;
pub(crate) const NORMALIZATION_EPSILON: f32 = 1.0e-4;

pub fn pathway_cfs() -> Vec<String> {
    vec![
        CF_MEJEPA_PATHWAY_TREES.to_string(),
        CF_MEJEPA_SURFACED_PATHWAYS.to_string(),
        CF_MEJEPA_OPERATOR_PATHWAY_CHOICES.to_string(),
    ]
}
