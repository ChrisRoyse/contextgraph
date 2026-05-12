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
pub const CF_MEJEPA_ORACLE_VERDICTS: &str = "CF_MEJEPA_ORACLE_VERDICTS";

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

// REQ-FLYWHEEL-10 — three new column families for the data flywheel.
//
// String values match the existing `CF_MEJEPA_*` upper-case convention so the
// "constant name == on-disk CF name" invariant continues to hold across the
// whole registry (the lower-case strings in TECH-FLYWHEEL.md §6 are spec
// illustrations only; the live mejepa-cf convention takes precedence).
pub const CF_MEJEPA_DDA_SIGNALS: &str = "CF_MEJEPA_DDA_SIGNALS";
pub const CF_MEJEPA_FAILURE_EXEMPLARS: &str = "CF_MEJEPA_FAILURE_EXEMPLARS";
pub const CF_MEJEPA_AGENT_FEEDBACK: &str = "CF_MEJEPA_AGENT_FEEDBACK";

/// REQ-FLYWHEEL-10 grouping. Daemon openers extend their CF descriptor list
/// with this slice so the three new CFs land on disk during the first daemon
/// startup, even if no other code path has touched them yet.
pub const FLYWHEEL_CFS: &[&str] = &[
    CF_MEJEPA_DDA_SIGNALS,
    CF_MEJEPA_FAILURE_EXEMPLARS,
    CF_MEJEPA_AGENT_FEEDBACK,
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
    CF_MEJEPA_ORACLE_VERDICTS,
    CF_MEJEPA_TRAIN_CERTS,
    CF_MEJEPA_SHIFT_WATERMARK,
    CF_MEJEPA_PANEL_CACHE,
    CF_MEJEPA_PANEL_META,
    CF_MEJEPA_AUX_SIGNALS_OVERRIDES,
    CF_MEJEPA_EVAL_REPORTS,
    CF_MEJEPA_TASK_GRAPH,
    CF_MEJEPA_TASK_GRAPH_HISTORY,
    CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
    CF_MEJEPA_ACTIVE_LEARNING_LABELS,
    CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS,
    CF_MEJEPA_OOD_ESCALATIONS,
    CF_MEJEPA_RESEARCH_QUESTIONS,
    CF_MEJEPA_MODEL_PROMOTIONS,
    CF_MEJEPA_PRODUCTION_TELEMETRY,
    CF_MEJEPA_DDA_SIGNALS,
    CF_MEJEPA_FAILURE_EXEMPLARS,
    CF_MEJEPA_AGENT_FEEDBACK,
];

pub const EVAL_CFS: &[&str] = &[
    CF_MEJEPA_EVAL_REPORTS,
    CF_MEJEPA_TASK_GRAPH,
    CF_MEJEPA_TASK_GRAPH_HISTORY,
    CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
    CF_MEJEPA_ACTIVE_LEARNING_LABELS,
    CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS,
    CF_MEJEPA_OOD_ESCALATIONS,
    CF_MEJEPA_RESEARCH_QUESTIONS,
    CF_MEJEPA_MODEL_PROMOTIONS,
    CF_MEJEPA_PRODUCTION_TELEMETRY,
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
    CF_MEJEPA_ACTIVE_LEARNING_LABELS,
    CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS,
    CF_MEJEPA_OOD_ESCALATIONS,
    CF_MEJEPA_RESEARCH_QUESTIONS,
    CF_MEJEPA_MODEL_PROMOTIONS,
    CF_MEJEPA_PRODUCTION_TELEMETRY,
];

pub const TCT_CFS: &[&str] = &[CF_MEJEPA_CONSTELLATION, CF_MEJEPA_GUARD_DECISIONS];

pub fn all_hygiene_referenced_cfs() -> Vec<&'static str> {
    let mut names = SELF_HEALING_REFERENCED_CFS.to_vec();
    for cf in INFER_CFS
        .iter()
        .chain(HYGIENE_CFS.iter())
        .chain(TCT_CFS.iter())
        .chain(FLYWHEEL_CFS.iter())
    {
        if !names.contains(cf) {
            names.push(*cf);
        }
    }
    names
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
    }
}
