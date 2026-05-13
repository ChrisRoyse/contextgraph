pub mod cell_exemptions;
pub mod error;
pub mod fixtures;
pub mod graph;
pub mod metrics;
pub mod mi_heatmap_renderer;
pub mod per_head_calibration_tracker;
pub mod queue;
pub mod report;
pub mod runner;
pub mod ship_gate_stability;
pub mod store;
pub mod telemetry;
pub mod types;

pub use cell_exemptions::{
    default_cell_exemptions_path, load_cell_exemptions, CellExemption, CellExemptionConfig,
    DEFAULT_CELL_EXEMPTIONS_PATH,
};
pub use error::{EvalError, EvalErrorCode};
pub use fixtures::{build_eval_compiler, synthetic_holdout, synthetic_patch_embeddings};
pub use graph::{
    build_patch_similarity_graph, PatchEmbedding, PatchGraphEdge, PatchSimilarityGraph,
};
pub use metrics::{
    compute_state_transfer_per_cell, conformal_health, ood_auc_by_language, pearson_correlation,
    state_transfer_from_observations,
};
pub use mi_heatmap_renderer::{
    load_pairwise_mi_heatmap_csv, render_pairwise_mi_heatmap, PairwiseMiHeatmapMatrix,
    PairwiseMiHeatmapRender,
};
pub use per_head_calibration_tracker::{
    compute_calibration_for_samples, compute_prediction_class_calibration, CalibrationSample,
    PredictionClassCalibration, PredictionClassCalibrationBin, DEFAULT_CALIBRATION_BIN_COUNT,
    DEFAULT_ECE_TOLERANCE,
};
pub use queue::{
    ActiveLearningKind, ActiveLearningLabel, ActiveLearningQueueEntry, ActiveLearningQueueState,
    LabelMethod,
};
pub use report::{seed_open_research_questions, write_weekly_report};
pub use runner::{corpus_sha_from_holdout, EvalRunner};
pub use ship_gate_stability::{
    effective_ship_gate_correlation, non_exempt_ship_gate_failures, ship_gate_stability_status,
    ship_gate_stability_status_with_exemptions, ship_gate_stability_status_with_requirements,
    ShipGateStabilityStatus, SHIP_GATE_REQUIRED_CONSECUTIVE_WINDOWS, SHIP_GATE_STABILITY_BLOCKER,
    SHIP_GATE_STABILITY_CORRELATION_THRESHOLD,
};
pub use store::RocksDbEvalStore;
pub use telemetry::{LatencyTelemetry, ProductionTelemetryWindow};
pub use types::{
    cell_key, language_slug, ActiveLearningSummary, AuxHeadDistillationSummary,
    ConformalHealthEntry, EvalConfig, EvalObservation, EvalProvenance, EvalReport, HoldoutPanel,
    MutationCategory, OpenResearchQuestionStatus, RegressionCheck, StateTransferDiagnostic,
};
