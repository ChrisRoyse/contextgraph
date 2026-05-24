pub mod ablation;
pub mod cell_exemptions;
pub mod convergence_tracker;
pub mod error;
pub mod fingerprint_ship_gate;
pub mod fixtures;
pub mod graph;
pub mod metrics;
pub mod mi_heatmap_renderer;
pub mod novel_pattern;
pub mod per_head_calibration_tracker;
pub mod queue;
pub mod report;
pub mod runner;
pub mod ship_gate_stability;
pub mod store;
pub mod telemetry;
pub mod types;

pub use ablation::{
    ablation_report_key, build_negative_action_ablation_report,
    incomplete_negative_action_ablation_report, negative_action_ablation_gate_status,
    render_ablation_weekly_markdown, write_ablation_weekly_markdown, AblationCellDrop,
    AblationReport, AblationRunInput, AblationVerdict, NegativeActionAblationGateStatus,
    ABLATION_INCOMPLETE, ABLATION_NUMERICAL_INSTABILITY, NEGATIVE_ACTION_ABLATION_BLOCKER,
    NEGATIVE_ACTION_ABLATION_CELL_DROP_THRESHOLD_PCT,
    NEGATIVE_ACTION_ABLATION_GLOBAL_DROP_THRESHOLD_PCT, NEGATIVE_ACTION_ABLATION_WARNING,
};
pub use cell_exemptions::{
    default_cell_exemptions_path, load_cell_exemptions, CellExemption, CellExemptionConfig,
    DEFAULT_CELL_EXEMPTIONS_PATH,
};
pub use convergence_tracker::{
    baseline_convergence_eta_for_cells, compute_convergence_eta_from_reports, CellConvergenceEta,
    ConvergenceEtaConfidenceInterval, ConvergenceEtaStatus, DEFAULT_CONVERGENCE_HISTORY_WINDOWS,
    DEFAULT_CONVERGENCE_MIN_POINTS, DEFAULT_CONVERGENCE_TARGET,
};
pub use error::{EvalError, EvalErrorCode};
pub use fingerprint_ship_gate::{
    fingerprint_ship_gate_stability_status,
    fingerprint_ship_gate_stability_status_with_requirements, FingerprintClassificationMetrics,
    FingerprintShipGateStabilityStatus, FingerprintShipGateWindow, UnknownOodRecallMetrics,
    FINGERPRINT_SHIP_GATE_ACCURACY_THRESHOLD, FINGERPRINT_SHIP_GATE_PRECISION_THRESHOLD,
    FINGERPRINT_SHIP_GATE_REQUIRED_CONSECUTIVE_WINDOWS, FINGERPRINT_SHIP_GATE_STABILITY_BLOCKER,
    FINGERPRINT_SHIP_GATE_UNKNOWN_OOD_RECALL_THRESHOLD,
};
// #694 / #594: `build_eval_compiler` and `synthetic_holdout` are
// fixture-only — they construct deterministic fakes for the MCP weekly
// dashboard regression-test surface (compiled only under
// `#[cfg(test)] mod fsv_tests` in `mejepa_weekly_dashboard_tools.rs`).
// Production entry points fail closed: `mejepa eval-run` at
// `bin/mejepa.rs:524` returns `FIXTURE_EVAL_DISABLED`; the MCP tool
// `mejepa_eval_run` at `mejepa_eval_tools.rs:111` returns
// `MEJEPA_EVAL_FIXTURE_PATH_DISABLED`. The re-export is kept `pub` here
// only for the `#[cfg(test)]` consumer; `#[doc(hidden)]` prevents the
// symbols from appearing in cargo-doc as public API, and the inline
// comment is the standing signal not to wire either symbol into a new
// production path. Future hardening: split these into a separate
// `context-graph-mejepa-test-fixtures` crate (follow-up filed under #694).
#[doc(hidden)]
pub use fixtures::{build_eval_compiler, synthetic_holdout};
pub use fixtures::synthetic_patch_embeddings;
pub use graph::{
    build_patch_similarity_graph, PatchEmbedding, PatchGraphEdge, PatchSimilarityGraph,
};
pub use metrics::{
    compute_failure_mode_class_metrics, compute_state_transfer_per_cell, conformal_health,
    empty_failure_mode_class_metrics, ood_auc_by_language, pearson_correlation,
    state_transfer_from_observations,
};
pub use mi_heatmap_renderer::{
    load_pairwise_mi_heatmap_csv, render_pairwise_mi_heatmap, PairwiseMiHeatmapMatrix,
    PairwiseMiHeatmapRender,
};
pub use novel_pattern::{
    admit_novel_pattern_clusters, candidate_audit_entry, evaluate_novel_pattern_promotion,
    ontology_growth_audit_key, proposal_inputs_from_ontology_audit, ConstellationCentroid,
    HeldoutImprovementEvidence, NovelInstrumentProposalInput, NovelPatternCandidate,
    NovelPatternClusterAdmission, NovelPatternClusterState, NovelPatternDetectorConfig,
    OntologyGrowthAuditEntry, OntologyGrowthAuditOutcome, OntologyGrowthInitiatedBy,
    DEFAULT_NOVEL_PATTERN_HELDOUT_DELTA, DEFAULT_NOVEL_PATTERN_MIN_CLUSTER_SIZE,
    DEFAULT_NOVEL_PATTERN_TAU_FAR, DEFAULT_NOVEL_PATTERN_TAU_INTRA,
    DEFAULT_NOVEL_PATTERN_TAU_NOVEL, NOVEL_PATTERN_SOURCE_CORPUS,
};
pub use per_head_calibration_tracker::{
    compute_calibration_for_samples, compute_prediction_class_calibration, CalibrationSample,
    PredictionClassCalibration, PredictionClassCalibrationBin, DEFAULT_CALIBRATION_BIN_COUNT,
    DEFAULT_ECE_TOLERANCE,
};
pub use queue::{
    conformal_width_proxy, curiosity_delta_matches, curiosity_score_from_proxy,
    curiosity_telemetry_window_key, render_curiosity_ranking_weekly_section, ActiveLearningKind,
    ActiveLearningLabel, ActiveLearningQueueEntry, ActiveLearningQueueState, ActiveLearningRankBy,
    CuriosityCalibrationEvidence, CuriosityDistribution, CuriosityRankedEntry,
    CuriosityTelemetryWindow, LabelMethod, UnknownFingerprintCandidate,
    UnknownFingerprintClusterSuggestion, CURIOSITY_TELEMETRY_SCHEMA_VERSION,
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
pub use telemetry::{
    training_holdout_distribution_drift, LatencyTelemetry, ProductionTelemetryWindow,
    TrainingHoldoutDistributionDrift, TRAINING_HOLDOUT_DRIFT_ALERT_CODE,
    TRAINING_HOLDOUT_DRIFT_KL_THRESHOLD, TRAINING_HOLDOUT_DRIFT_REQUIRED_BATCHES,
};
pub use types::{
    cell_key, language_slug, required_active_python_ship_gate_cells,
    validate_active_python_ship_gate_report, ActiveLearningSummary, AuxHeadDistillationSummary,
    ConformalHealthEntry, EvalConfig, EvalObservation, EvalProvenance, EvalReport,
    FailureModeClassMetrics, HoldoutPanel, MutationCategory, OpenResearchQuestionStatus,
    RegressionCheck, StateTransferDiagnostic, ACTIVE_PYTHON_SHIP_GATE_CELL_COUNT,
    ACTIVE_PYTHON_SHIP_GATE_GRID, ACTIVE_PYTHON_SHIP_GATE_NAME,
};
