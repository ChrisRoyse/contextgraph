pub mod error;
pub mod fixtures;
pub mod graph;
pub mod metrics;
pub mod queue;
pub mod report;
pub mod runner;
pub mod store;
pub mod telemetry;
pub mod types;

pub use error::{EvalError, EvalErrorCode};
pub use fixtures::{build_eval_compiler, synthetic_holdout, synthetic_patch_embeddings};
pub use graph::{
    build_patch_similarity_graph, PatchEmbedding, PatchGraphEdge, PatchSimilarityGraph,
};
pub use metrics::{conformal_health, ood_auc_by_language, pearson_correlation};
pub use queue::{
    ActiveLearningKind, ActiveLearningLabel, ActiveLearningQueueEntry, ActiveLearningQueueState,
    LabelMethod,
};
pub use report::{seed_open_research_questions, write_weekly_report};
pub use runner::{corpus_sha_from_holdout, EvalRunner};
pub use store::RocksDbEvalStore;
pub use telemetry::{LatencyTelemetry, ProductionTelemetryWindow};
pub use types::{
    ActiveLearningSummary, AuxHeadDistillationSummary, ConformalHealthEntry, EvalConfig,
    EvalObservation, EvalProvenance, EvalReport, HoldoutPanel, MutationCategory,
    OpenResearchQuestionStatus, RegressionCheck, StateTransferDiagnostic,
};
