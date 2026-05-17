//! Canonical ME-JEPA RocksDB column-family names.
//!
//! This crate is intentionally dependency-free so ME-JEPA runtime, hygiene,
//! MCP, and training crates can share CF names without cycles or string drift.

pub const CF_MEJEPA_FISHER_SNAPSHOTS: &str = "CF_MEJEPA_FISHER_SNAPSHOTS";
pub const CF_MEJEPA_DRIFT_WINDOW: &str = "CF_MEJEPA_DRIFT_WINDOW";
pub const CF_MEJEPA_DRIFT_HISTORY: &str = "CF_MEJEPA_DRIFT_HISTORY";
pub const CF_MEJEPA_HEAL_REPORTS: &str = "CF_MEJEPA_HEAL_REPORTS";
pub const CF_MEJEPA_DISTILL_STEPS: &str = "CF_MEJEPA_DISTILL_STEPS";
pub const CF_MEJEPA_ACTIVE_POINTERS: &str = "CF_MEJEPA_ACTIVE_POINTERS";
pub const CF_MEJEPA_CALIBRATION_HISTORY: &str = "CF_MEJEPA_CALIBRATION_HISTORY";
pub const CF_MEJEPA_WEIGHT_BLOBS: &str = "CF_MEJEPA_WEIGHT_BLOBS";
pub const CF_MEJEPA_SHIFT_WATERMARK: &str = "CF_MEJEPA_SHIFT_WATERMARK";
pub const CF_MEJEPA_TRAIN_CERTS: &str = "CF_MEJEPA_TRAIN_CERTS";
pub const CF_MEJEPA_LIVE_PREDICTIONS: &str = "CF_MEJEPA_LIVE_PREDICTIONS";
// TASK-FP-102 — durable file/function/AST/chunk prediction hierarchy rows.
pub const CF_MEJEPA_HIERARCHICAL_PREDICTIONS: &str = "CF_MEJEPA_HIERARCHICAL_PREDICTIONS";
pub const CF_MEJEPA_ORACLE_VERDICTS: &str = "CF_MEJEPA_ORACLE_VERDICTS";
pub const CF_MEJEPA_PANELS: &str = "CF_MEJEPA_PANELS";

pub const CF_MEJEPA_MUTATION_CORPUS: &str = "CF_MEJEPA_MUTATION_CORPUS";
pub const CF_MEJEPA_LIVE_ATTEMPT_CORPUS: &str = "CF_MEJEPA_LIVE_ATTEMPT_CORPUS";
pub const CF_MEJEPA_PANEL_META: &str = "CF_MEJEPA_PANEL_META";
pub const CF_MEJEPA_QUOTA_STATE: &str = "CF_MEJEPA_QUOTA_STATE";
pub const CF_MEJEPA_WEIGHT_BLOBS_META: &str = "CF_MEJEPA_WEIGHT_BLOBS_META";
pub const CF_MEJEPA_WITNESS_CHAIN: &str = "CF_MEJEPA_WITNESS_CHAIN";
pub const CF_MEJEPA_WITNESS_SEGMENT_META: &str = "CF_MEJEPA_WITNESS_SEGMENT_META";
pub const CF_MEJEPA_PANEL_CACHE: &str = "CF_MEJEPA_PANEL_CACHE";
pub const CF_MEJEPA_PANEL_DISK_OVERFLOW: &str = "CF_MEJEPA_PANEL_DISK_OVERFLOW";
pub const CF_MEJEPA_AUX_SIGNALS_OVERRIDES: &str = "CF_MEJEPA_AUX_SIGNALS_OVERRIDES";
pub const CF_MEJEPA_GC_HISTORY: &str = "CF_MEJEPA_GC_HISTORY";
pub const CF_MEJEPA_EVAL_REPORTS: &str = "CF_MEJEPA_EVAL_REPORTS";
pub const CF_MEJEPA_TASK_GRAPH: &str = "CF_MEJEPA_TASK_GRAPH";
pub const CF_MEJEPA_TASK_GRAPH_HISTORY: &str = "CF_MEJEPA_TASK_GRAPH_HISTORY";
pub const CF_MEJEPA_ACTIVE_LEARNING_QUEUE: &str = "CF_MEJEPA_ACTIVE_LEARNING_QUEUE";
pub const CF_MEJEPA_ACTIVE_LEARNING_LABELS: &str = "CF_MEJEPA_ACTIVE_LEARNING_LABELS";
pub const CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS: &str = "CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS";
pub const CF_MEJEPA_OOD_ESCALATIONS: &str = "CF_MEJEPA_OOD_ESCALATIONS";
pub const CF_MEJEPA_RESEARCH_QUESTIONS: &str = "CF_MEJEPA_RESEARCH_QUESTIONS";
pub const CF_MEJEPA_MODEL_PROMOTIONS: &str = "CF_MEJEPA_MODEL_PROMOTIONS";
pub const CF_MEJEPA_PRODUCTION_TELEMETRY: &str = "CF_MEJEPA_PRODUCTION_TELEMETRY";
pub const CF_MEJEPA_CONSTELLATION: &str = "CF_MEJEPA_CONSTELLATION";
pub const CF_MEJEPA_GUARD_DECISIONS: &str = "CF_MEJEPA_GUARD_DECISIONS";
pub const CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS: &str = "CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS";
pub const CF_MEJEPA_CONSTELLATION_REFRESH_LOG: &str = "CF_MEJEPA_CONSTELLATION_REFRESH_LOG";

// REQ-FLYWHEEL-10 — three new column families for the data flywheel.
//
// String values match the existing `CF_MEJEPA_*` upper-case convention so the
// "constant name == on-disk CF name" invariant continues to hold across the
// whole registry (the lower-case strings in TECH-FLYWHEEL.md §6 are spec
// illustrations only; the live mejepa-cf convention takes precedence).
pub const CF_MEJEPA_DDA_SIGNALS: &str = "CF_MEJEPA_DDA_SIGNALS";
pub const CF_MEJEPA_FAILURE_EXEMPLARS: &str = "CF_MEJEPA_FAILURE_EXEMPLARS";
pub const CF_MEJEPA_AGENT_FEEDBACK: &str = "CF_MEJEPA_AGENT_FEEDBACK";
pub const CF_MEJEPA_FLYWHEEL_JOB_RUNS: &str = "CF_MEJEPA_FLYWHEEL_JOB_RUNS";
// REQ-PREDICT-12 — operator override records, keyed by prediction_id.
pub const CF_MEJEPA_OPERATOR_OVERRIDES: &str = "CF_MEJEPA_OPERATOR_OVERRIDES";
// TASK-PREDICT-LABEL-012 — durable decisions for every attempted label transfer.
pub const CF_MEJEPA_LABEL_TRANSFER_DECISIONS: &str = "CF_MEJEPA_LABEL_TRANSFER_DECISIONS";
// TASK-FLYWHEEL-007 — fail-closed retry park-list records, keyed by prediction_id.
pub const CF_MEJEPA_PARK_LIST: &str = "CF_MEJEPA_PARK_LIST";
// TASK-PY-G-061 — cold constellation cells must abstain and emit durable metrics.
pub const CF_MEJEPA_COLD_CELL_METRICS: &str = "CF_MEJEPA_COLD_CELL_METRICS";
// TASK-RWD-208 / TASK-PY-G-074 — normalized mypy/pyright/ruff labels.
pub const CF_MEJEPA_STATIC_ANALYSIS_SIGNALS: &str = "CF_MEJEPA_STATIC_ANALYSIS_SIGNALS";
// TASK-RWD-204 / TASK-PY-G-046 — normalized Bandit/Semgrep Q4 security labels.
pub const CF_MEJEPA_Q4_SECURITY_LABELS: &str = "CF_MEJEPA_Q4_SECURITY_LABELS";
// TASK-RWD-209 / TASK-PY-G-075 — composite reward-signal labels.
pub const CF_MEJEPA_COVERAGE_SIGNALS: &str = "CF_MEJEPA_COVERAGE_SIGNALS";
pub const CF_MEJEPA_COMPLEXITY_SIGNALS: &str = "CF_MEJEPA_COMPLEXITY_SIGNALS";
pub const CF_MEJEPA_CHURN_SIGNALS: &str = "CF_MEJEPA_CHURN_SIGNALS";
pub const CF_MEJEPA_DOC_COVERAGE_SIGNALS: &str = "CF_MEJEPA_DOC_COVERAGE_SIGNALS";
pub const CF_MEJEPA_DEP_CVE_SIGNALS: &str = "CF_MEJEPA_DEP_CVE_SIGNALS";
// TASK-RWD-210 / TASK-PY-G-068 — prioritized M(t) replay buffer rows.
pub const CF_MEJEPA_REPLAY_BUFFER: &str = "CF_MEJEPA_REPLAY_BUFFER";
// TASK-RWD-211 / TASK-PY-G-067 — accepted/refuted system-proposed
// constellation/fingerprint growth rows from the novel-pattern gate.
pub const CF_MEJEPA_ONTOLOGY_GROWTH_AUDIT: &str = "CF_MEJEPA_ONTOLOGY_GROWTH_AUDIT";
// TASK-FP-001/002 — first-class failure-shape fingerprint catalog.
pub const CF_MEJEPA_FAILURE_FINGERPRINTS: &str = "CF_MEJEPA_FAILURE_FINGERPRINTS";
pub const CF_MEJEPA_FINGERPRINT_REFERENCES: &str = "CF_MEJEPA_FINGERPRINT_REFERENCES";
pub const CF_MEJEPA_FINGERPRINT_REVERSE_INDEX: &str = "CF_MEJEPA_FINGERPRINT_REVERSE_INDEX";
pub const CF_MEJEPA_FINGERPRINT_CALIBRATION: &str = "CF_MEJEPA_FINGERPRINT_CALIBRATION";
pub const CF_MEJEPA_FINGERPRINT_AUDIT: &str = "CF_MEJEPA_FINGERPRINT_AUDIT";
pub const CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS: &str = "CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS";
// TASK-FP-008 — per-fingerprint EWC Fisher diagonal + CBP dormancy snapshots for
// catalog stability under continual learning (issue #317).
pub const CF_MEJEPA_FINGERPRINT_FISHER: &str = "CF_MEJEPA_FINGERPRINT_FISHER";
pub const CF_MEJEPA_FINGERPRINT_DORMANCY: &str = "CF_MEJEPA_FINGERPRINT_DORMANCY";
// TASK-PY-G-060 / TASK-FP-009 — 30-case adversarial Python corpus rows that
// feed the first-class failure-shape fingerprint catalog.
pub const CF_MEJEPA_ADVERSARIAL_CORPUS: &str = "CF_MEJEPA_ADVERSARIAL_CORPUS";

/// REQ-FLYWHEEL-10 grouping. Daemon openers extend their CF descriptor list
/// with this slice so the three new CFs land on disk during the first daemon
/// startup, even if no other code path has touched them yet.
pub const FLYWHEEL_CFS: &[&str] = &[
    CF_MEJEPA_DDA_SIGNALS,
    CF_MEJEPA_FAILURE_EXEMPLARS,
    CF_MEJEPA_AGENT_FEEDBACK,
    CF_MEJEPA_FLYWHEEL_JOB_RUNS,
    CF_MEJEPA_PARK_LIST,
];

pub const FINGERPRINT_CFS: &[&str] = &[
    CF_MEJEPA_FAILURE_FINGERPRINTS,
    CF_MEJEPA_FINGERPRINT_REFERENCES,
    CF_MEJEPA_FINGERPRINT_REVERSE_INDEX,
    CF_MEJEPA_FINGERPRINT_CALIBRATION,
    CF_MEJEPA_FINGERPRINT_AUDIT,
    CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS,
    CF_MEJEPA_FINGERPRINT_FISHER,
    CF_MEJEPA_FINGERPRINT_DORMANCY,
];

pub const ADVERSARIAL_CFS: &[&str] = &[CF_MEJEPA_ADVERSARIAL_CORPUS];

pub const REWARD_SIGNAL_CFS: &[&str] = &[
    CF_MEJEPA_COVERAGE_SIGNALS,
    CF_MEJEPA_COMPLEXITY_SIGNALS,
    CF_MEJEPA_CHURN_SIGNALS,
    CF_MEJEPA_DOC_COVERAGE_SIGNALS,
    CF_MEJEPA_DEP_CVE_SIGNALS,
    CF_MEJEPA_REPLAY_BUFFER,
    CF_MEJEPA_ONTOLOGY_GROWTH_AUDIT,
    CF_MEJEPA_CONSTELLATION_REFRESH_LOG,
];

pub const SELF_HEALING_CFS: &[&str] = &[
    CF_MEJEPA_FISHER_SNAPSHOTS,
    CF_MEJEPA_DRIFT_WINDOW,
    CF_MEJEPA_DRIFT_HISTORY,
    CF_MEJEPA_HEAL_REPORTS,
    CF_MEJEPA_DISTILL_STEPS,
    CF_MEJEPA_ACTIVE_POINTERS,
    CF_MEJEPA_CALIBRATION_HISTORY,
];

pub const SELF_HEALING_REFERENCED_CFS: &[&str] = &[
    CF_MEJEPA_FISHER_SNAPSHOTS,
    CF_MEJEPA_DRIFT_WINDOW,
    CF_MEJEPA_DRIFT_HISTORY,
    CF_MEJEPA_HEAL_REPORTS,
    CF_MEJEPA_DISTILL_STEPS,
    CF_MEJEPA_ACTIVE_POINTERS,
    CF_MEJEPA_CALIBRATION_HISTORY,
    CF_MEJEPA_WEIGHT_BLOBS,
    CF_MEJEPA_SHIFT_WATERMARK,
    CF_MEJEPA_TRAIN_CERTS,
];

pub const INFER_CFS: &[&str] = &[
    CF_MEJEPA_CALIBRATION_HISTORY,
    CF_MEJEPA_LIVE_PREDICTIONS,
    CF_MEJEPA_HIERARCHICAL_PREDICTIONS,
    CF_MEJEPA_ORACLE_VERDICTS,
    CF_MEJEPA_TRAIN_CERTS,
    CF_MEJEPA_PANELS,
    CF_MEJEPA_SHIFT_WATERMARK,
    CF_MEJEPA_PANEL_CACHE,
    CF_MEJEPA_PANEL_META,
    CF_MEJEPA_AUX_SIGNALS_OVERRIDES,
    CF_MEJEPA_EVAL_REPORTS,
    CF_MEJEPA_TASK_GRAPH,
    CF_MEJEPA_TASK_GRAPH_HISTORY,
    CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
    CF_MEJEPA_FAILURE_FINGERPRINTS,
    CF_MEJEPA_FINGERPRINT_REFERENCES,
    CF_MEJEPA_FINGERPRINT_REVERSE_INDEX,
    CF_MEJEPA_FINGERPRINT_CALIBRATION,
    CF_MEJEPA_FINGERPRINT_AUDIT,
    CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS,
    CF_MEJEPA_FINGERPRINT_FISHER,
    CF_MEJEPA_FINGERPRINT_DORMANCY,
    CF_MEJEPA_ADVERSARIAL_CORPUS,
    CF_MEJEPA_ACTIVE_LEARNING_LABELS,
    CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS,
    CF_MEJEPA_OOD_ESCALATIONS,
    CF_MEJEPA_RESEARCH_QUESTIONS,
    CF_MEJEPA_MODEL_PROMOTIONS,
    CF_MEJEPA_PRODUCTION_TELEMETRY,
    CF_MEJEPA_DDA_SIGNALS,
    CF_MEJEPA_FAILURE_EXEMPLARS,
    CF_MEJEPA_AGENT_FEEDBACK,
    CF_MEJEPA_FLYWHEEL_JOB_RUNS,
    CF_MEJEPA_OPERATOR_OVERRIDES,
    CF_MEJEPA_LABEL_TRANSFER_DECISIONS,
    CF_MEJEPA_PARK_LIST,
    CF_MEJEPA_COLD_CELL_METRICS,
    CF_MEJEPA_STATIC_ANALYSIS_SIGNALS,
    CF_MEJEPA_Q4_SECURITY_LABELS,
    CF_MEJEPA_COVERAGE_SIGNALS,
    CF_MEJEPA_COMPLEXITY_SIGNALS,
    CF_MEJEPA_CHURN_SIGNALS,
    CF_MEJEPA_DOC_COVERAGE_SIGNALS,
    CF_MEJEPA_DEP_CVE_SIGNALS,
    CF_MEJEPA_REPLAY_BUFFER,
    CF_MEJEPA_ONTOLOGY_GROWTH_AUDIT,
    CF_MEJEPA_CONSTELLATION_REFRESH_LOG,
];

pub const EVAL_CFS: &[&str] = &[
    CF_MEJEPA_EVAL_REPORTS,
    CF_MEJEPA_TASK_GRAPH,
    CF_MEJEPA_TASK_GRAPH_HISTORY,
    CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
    CF_MEJEPA_FAILURE_FINGERPRINTS,
    CF_MEJEPA_FINGERPRINT_REFERENCES,
    CF_MEJEPA_FINGERPRINT_REVERSE_INDEX,
    CF_MEJEPA_FINGERPRINT_CALIBRATION,
    CF_MEJEPA_FINGERPRINT_AUDIT,
    CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS,
    CF_MEJEPA_ADVERSARIAL_CORPUS,
    CF_MEJEPA_ACTIVE_LEARNING_LABELS,
    CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS,
    CF_MEJEPA_OOD_ESCALATIONS,
    CF_MEJEPA_RESEARCH_QUESTIONS,
    CF_MEJEPA_MODEL_PROMOTIONS,
    CF_MEJEPA_PRODUCTION_TELEMETRY,
    CF_MEJEPA_FLYWHEEL_JOB_RUNS,
    CF_MEJEPA_LABEL_TRANSFER_DECISIONS,
    CF_MEJEPA_PARK_LIST,
    CF_MEJEPA_COLD_CELL_METRICS,
    CF_MEJEPA_STATIC_ANALYSIS_SIGNALS,
    CF_MEJEPA_Q4_SECURITY_LABELS,
    CF_MEJEPA_COVERAGE_SIGNALS,
    CF_MEJEPA_COMPLEXITY_SIGNALS,
    CF_MEJEPA_CHURN_SIGNALS,
    CF_MEJEPA_DOC_COVERAGE_SIGNALS,
    CF_MEJEPA_DEP_CVE_SIGNALS,
    CF_MEJEPA_REPLAY_BUFFER,
    CF_MEJEPA_ONTOLOGY_GROWTH_AUDIT,
];

pub const HYGIENE_CFS: &[&str] = &[
    CF_MEJEPA_MUTATION_CORPUS,
    CF_MEJEPA_LIVE_ATTEMPT_CORPUS,
    CF_MEJEPA_PANEL_META,
    CF_MEJEPA_QUOTA_STATE,
    CF_MEJEPA_WEIGHT_BLOBS,
    CF_MEJEPA_WEIGHT_BLOBS_META,
    CF_MEJEPA_WITNESS_CHAIN,
    CF_MEJEPA_WITNESS_SEGMENT_META,
    CF_MEJEPA_PANEL_CACHE,
    CF_MEJEPA_PANEL_DISK_OVERFLOW,
    CF_MEJEPA_CALIBRATION_HISTORY,
    CF_MEJEPA_AUX_SIGNALS_OVERRIDES,
    CF_MEJEPA_SHIFT_WATERMARK,
    CF_MEJEPA_TRAIN_CERTS,
    CF_MEJEPA_GC_HISTORY,
    CF_MEJEPA_EVAL_REPORTS,
    CF_MEJEPA_TASK_GRAPH,
    CF_MEJEPA_TASK_GRAPH_HISTORY,
    CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
    CF_MEJEPA_HIERARCHICAL_PREDICTIONS,
    CF_MEJEPA_FAILURE_FINGERPRINTS,
    CF_MEJEPA_FINGERPRINT_REFERENCES,
    CF_MEJEPA_FINGERPRINT_REVERSE_INDEX,
    CF_MEJEPA_FINGERPRINT_CALIBRATION,
    CF_MEJEPA_FINGERPRINT_AUDIT,
    CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS,
    CF_MEJEPA_FINGERPRINT_FISHER,
    CF_MEJEPA_FINGERPRINT_DORMANCY,
    CF_MEJEPA_ADVERSARIAL_CORPUS,
    CF_MEJEPA_ACTIVE_LEARNING_LABELS,
    CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS,
    CF_MEJEPA_OOD_ESCALATIONS,
    CF_MEJEPA_RESEARCH_QUESTIONS,
    CF_MEJEPA_MODEL_PROMOTIONS,
    CF_MEJEPA_PRODUCTION_TELEMETRY,
    CF_MEJEPA_LABEL_TRANSFER_DECISIONS,
    CF_MEJEPA_COLD_CELL_METRICS,
    CF_MEJEPA_STATIC_ANALYSIS_SIGNALS,
    CF_MEJEPA_Q4_SECURITY_LABELS,
    CF_MEJEPA_COVERAGE_SIGNALS,
    CF_MEJEPA_COMPLEXITY_SIGNALS,
    CF_MEJEPA_CHURN_SIGNALS,
    CF_MEJEPA_DOC_COVERAGE_SIGNALS,
    CF_MEJEPA_DEP_CVE_SIGNALS,
    CF_MEJEPA_REPLAY_BUFFER,
    CF_MEJEPA_CONSTELLATION_REFRESH_LOG,
];

pub const TCT_CFS: &[&str] = &[
    CF_MEJEPA_CONSTELLATION,
    CF_MEJEPA_GUARD_DECISIONS,
    CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS,
    CF_MEJEPA_CONSTELLATION_REFRESH_LOG,
];

/// Canonical, duplicate-free list opened by ME-JEPA hygiene/runtime databases.
///
/// This is intentionally explicit instead of synthesized at runtime: downstream
/// storage code uses the length as part of the full RocksDB schema contract, so
/// adding or removing a CF must be a visible schema change.
pub const ALL_HYGIENE_REFERENCED_CFS: &[&str] = &[
    CF_MEJEPA_FISHER_SNAPSHOTS,
    CF_MEJEPA_DRIFT_WINDOW,
    CF_MEJEPA_DRIFT_HISTORY,
    CF_MEJEPA_HEAL_REPORTS,
    CF_MEJEPA_DISTILL_STEPS,
    CF_MEJEPA_ACTIVE_POINTERS,
    CF_MEJEPA_CALIBRATION_HISTORY,
    CF_MEJEPA_WEIGHT_BLOBS,
    CF_MEJEPA_SHIFT_WATERMARK,
    CF_MEJEPA_TRAIN_CERTS,
    CF_MEJEPA_LIVE_PREDICTIONS,
    CF_MEJEPA_HIERARCHICAL_PREDICTIONS,
    CF_MEJEPA_ORACLE_VERDICTS,
    CF_MEJEPA_PANELS,
    CF_MEJEPA_PANEL_CACHE,
    CF_MEJEPA_PANEL_META,
    CF_MEJEPA_AUX_SIGNALS_OVERRIDES,
    CF_MEJEPA_EVAL_REPORTS,
    CF_MEJEPA_TASK_GRAPH,
    CF_MEJEPA_TASK_GRAPH_HISTORY,
    CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
    CF_MEJEPA_FAILURE_FINGERPRINTS,
    CF_MEJEPA_FINGERPRINT_REFERENCES,
    CF_MEJEPA_FINGERPRINT_REVERSE_INDEX,
    CF_MEJEPA_FINGERPRINT_CALIBRATION,
    CF_MEJEPA_FINGERPRINT_AUDIT,
    CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS,
    CF_MEJEPA_FINGERPRINT_FISHER,
    CF_MEJEPA_FINGERPRINT_DORMANCY,
    CF_MEJEPA_ADVERSARIAL_CORPUS,
    CF_MEJEPA_ACTIVE_LEARNING_LABELS,
    CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS,
    CF_MEJEPA_OOD_ESCALATIONS,
    CF_MEJEPA_RESEARCH_QUESTIONS,
    CF_MEJEPA_MODEL_PROMOTIONS,
    CF_MEJEPA_PRODUCTION_TELEMETRY,
    CF_MEJEPA_DDA_SIGNALS,
    CF_MEJEPA_FAILURE_EXEMPLARS,
    CF_MEJEPA_AGENT_FEEDBACK,
    CF_MEJEPA_FLYWHEEL_JOB_RUNS,
    CF_MEJEPA_OPERATOR_OVERRIDES,
    CF_MEJEPA_LABEL_TRANSFER_DECISIONS,
    CF_MEJEPA_PARK_LIST,
    CF_MEJEPA_COLD_CELL_METRICS,
    CF_MEJEPA_STATIC_ANALYSIS_SIGNALS,
    CF_MEJEPA_Q4_SECURITY_LABELS,
    CF_MEJEPA_COVERAGE_SIGNALS,
    CF_MEJEPA_COMPLEXITY_SIGNALS,
    CF_MEJEPA_CHURN_SIGNALS,
    CF_MEJEPA_DOC_COVERAGE_SIGNALS,
    CF_MEJEPA_DEP_CVE_SIGNALS,
    CF_MEJEPA_REPLAY_BUFFER,
    CF_MEJEPA_ONTOLOGY_GROWTH_AUDIT,
    CF_MEJEPA_CONSTELLATION_REFRESH_LOG,
    CF_MEJEPA_MUTATION_CORPUS,
    CF_MEJEPA_LIVE_ATTEMPT_CORPUS,
    CF_MEJEPA_QUOTA_STATE,
    CF_MEJEPA_WEIGHT_BLOBS_META,
    CF_MEJEPA_WITNESS_CHAIN,
    CF_MEJEPA_WITNESS_SEGMENT_META,
    CF_MEJEPA_PANEL_DISK_OVERFLOW,
    CF_MEJEPA_GC_HISTORY,
    CF_MEJEPA_CONSTELLATION,
    CF_MEJEPA_GUARD_DECISIONS,
    CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS,
];

pub const MEJEPA_HYGIENE_REFERENCED_CF_COUNT: usize = ALL_HYGIENE_REFERENCED_CFS.len();

pub fn all_hygiene_referenced_cfs() -> Vec<&'static str> {
    ALL_HYGIENE_REFERENCED_CFS.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_duplicate_hygiene_cfs() {
        let names = all_hygiene_referenced_cfs();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(names.len(), sorted.len());
        assert_eq!(names.len(), MEJEPA_HYGIENE_REFERENCED_CF_COUNT);
    }

    #[test]
    fn canonical_hygiene_cfs_match_group_union() {
        let mut generated = SELF_HEALING_REFERENCED_CFS.to_vec();
        for cf in INFER_CFS
            .iter()
            .chain(HYGIENE_CFS.iter())
            .chain(TCT_CFS.iter())
            .chain(FLYWHEEL_CFS.iter())
            .chain(FINGERPRINT_CFS.iter())
            .chain(ADVERSARIAL_CFS.iter())
            .chain(REWARD_SIGNAL_CFS.iter())
        {
            if !generated.contains(cf) {
                generated.push(*cf);
            }
        }

        assert_eq!(
            ALL_HYGIENE_REFERENCED_CFS,
            generated.as_slice(),
            "canonical ME-JEPA CF list drifted from the grouped CF contract",
        );
    }

    #[test]
    fn flywheel_cfs_are_registered_in_daemon_opener() {
        let names = all_hygiene_referenced_cfs();
        for cf in FLYWHEEL_CFS {
            assert!(
                names.contains(cf),
                "FLYWHEEL CF {cf} missing from daemon opener; \
                 daemon startup would fail to create the column family on \
                 first run and downstream put_cf calls would panic.",
            );
        }
    }

    #[test]
    fn flywheel_cf_names_match_const_idents() {
        // REQ-FLYWHEEL-10 contract: the on-disk CF name MUST equal the
        // constant identifier, so renaming the constant forces a deliberate
        // (and grep-visible) data migration rather than a silent rename.
        assert_eq!(CF_MEJEPA_DDA_SIGNALS, "CF_MEJEPA_DDA_SIGNALS");
        assert_eq!(CF_MEJEPA_FAILURE_EXEMPLARS, "CF_MEJEPA_FAILURE_EXEMPLARS");
        assert_eq!(CF_MEJEPA_AGENT_FEEDBACK, "CF_MEJEPA_AGENT_FEEDBACK");
        assert_eq!(CF_MEJEPA_FLYWHEEL_JOB_RUNS, "CF_MEJEPA_FLYWHEEL_JOB_RUNS");
        assert_eq!(
            CF_MEJEPA_HIERARCHICAL_PREDICTIONS,
            "CF_MEJEPA_HIERARCHICAL_PREDICTIONS"
        );
        assert_eq!(
            CF_MEJEPA_LABEL_TRANSFER_DECISIONS,
            "CF_MEJEPA_LABEL_TRANSFER_DECISIONS"
        );
        assert_eq!(CF_MEJEPA_COLD_CELL_METRICS, "CF_MEJEPA_COLD_CELL_METRICS");
        assert_eq!(
            CF_MEJEPA_STATIC_ANALYSIS_SIGNALS,
            "CF_MEJEPA_STATIC_ANALYSIS_SIGNALS"
        );
        assert_eq!(CF_MEJEPA_Q4_SECURITY_LABELS, "CF_MEJEPA_Q4_SECURITY_LABELS");
        assert_eq!(CF_MEJEPA_COVERAGE_SIGNALS, "CF_MEJEPA_COVERAGE_SIGNALS");
        assert_eq!(CF_MEJEPA_COMPLEXITY_SIGNALS, "CF_MEJEPA_COMPLEXITY_SIGNALS");
        assert_eq!(CF_MEJEPA_CHURN_SIGNALS, "CF_MEJEPA_CHURN_SIGNALS");
        assert_eq!(
            CF_MEJEPA_DOC_COVERAGE_SIGNALS,
            "CF_MEJEPA_DOC_COVERAGE_SIGNALS"
        );
        assert_eq!(CF_MEJEPA_DEP_CVE_SIGNALS, "CF_MEJEPA_DEP_CVE_SIGNALS");
        assert_eq!(CF_MEJEPA_REPLAY_BUFFER, "CF_MEJEPA_REPLAY_BUFFER");
        assert_eq!(
            CF_MEJEPA_ONTOLOGY_GROWTH_AUDIT,
            "CF_MEJEPA_ONTOLOGY_GROWTH_AUDIT"
        );
    }

    #[test]
    fn fingerprint_cfs_are_registered_in_daemon_opener() {
        let names = all_hygiene_referenced_cfs();
        for cf in FINGERPRINT_CFS {
            assert!(
                names.contains(cf),
                "fingerprint CF {cf} missing from daemon opener; seed catalog writes would miss the source-of-truth column family.",
            );
        }
    }

    #[test]
    fn fingerprint_cf_names_match_const_idents() {
        assert_eq!(
            CF_MEJEPA_FAILURE_FINGERPRINTS,
            "CF_MEJEPA_FAILURE_FINGERPRINTS"
        );
        assert_eq!(
            CF_MEJEPA_FINGERPRINT_REFERENCES,
            "CF_MEJEPA_FINGERPRINT_REFERENCES"
        );
        assert_eq!(
            CF_MEJEPA_FINGERPRINT_REVERSE_INDEX,
            "CF_MEJEPA_FINGERPRINT_REVERSE_INDEX"
        );
        assert_eq!(
            CF_MEJEPA_FINGERPRINT_CALIBRATION,
            "CF_MEJEPA_FINGERPRINT_CALIBRATION"
        );
        assert_eq!(CF_MEJEPA_FINGERPRINT_AUDIT, "CF_MEJEPA_FINGERPRINT_AUDIT");
        assert_eq!(
            CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS,
            "CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS"
        );
        assert_eq!(CF_MEJEPA_FINGERPRINT_FISHER, "CF_MEJEPA_FINGERPRINT_FISHER");
        assert_eq!(
            CF_MEJEPA_FINGERPRINT_DORMANCY,
            "CF_MEJEPA_FINGERPRINT_DORMANCY"
        );
        assert_eq!(CF_MEJEPA_ADVERSARIAL_CORPUS, "CF_MEJEPA_ADVERSARIAL_CORPUS");
    }

    #[test]
    fn fingerprint_fisher_and_dormancy_cfs_are_registered_in_daemon_opener() {
        // TASK-FP-008 — both new CFs must land on the daemon opener slice or
        // the fingerprint stability snapshots will be silently dropped at
        // production runtime.
        let names = all_hygiene_referenced_cfs();
        assert!(
            names.contains(&CF_MEJEPA_FINGERPRINT_FISHER),
            "CF_MEJEPA_FINGERPRINT_FISHER missing from daemon opener; per-fingerprint EWC Fisher rows would not persist."
        );
        assert!(
            names.contains(&CF_MEJEPA_FINGERPRINT_DORMANCY),
            "CF_MEJEPA_FINGERPRINT_DORMANCY missing from daemon opener; per-fingerprint CBP dormancy rows would not persist."
        );
        // Also confirm the FINGERPRINT_CFS group is the single canonical list.
        assert!(FINGERPRINT_CFS.contains(&CF_MEJEPA_FINGERPRINT_FISHER));
        assert!(FINGERPRINT_CFS.contains(&CF_MEJEPA_FINGERPRINT_DORMANCY));
    }

    #[test]
    fn adversarial_cfs_are_registered_in_daemon_opener() {
        let names = all_hygiene_referenced_cfs();
        for cf in ADVERSARIAL_CFS {
            assert!(
                names.contains(cf),
                "adversarial CF {cf} missing from daemon opener; TASK-PY-G-060 rows would not persist.",
            );
        }
    }

    #[test]
    fn reward_signal_cfs_are_registered_in_daemon_opener() {
        let names = all_hygiene_referenced_cfs();
        for cf in REWARD_SIGNAL_CFS {
            assert!(
                names.contains(cf),
                "reward signal CF {cf} missing from daemon opener; TASK-RWD-209 rows would not persist.",
            );
        }
    }

    #[test]
    fn tct_cfs_are_registered_in_daemon_opener() {
        let names = all_hygiene_referenced_cfs();
        for cf in TCT_CFS {
            assert!(
                names.contains(cf),
                "TCT CF {cf} missing from daemon opener; production TCT source-of-truth reads would miss persisted rows.",
            );
        }
    }
}
