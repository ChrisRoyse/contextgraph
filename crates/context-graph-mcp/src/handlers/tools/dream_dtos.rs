//! DTOs for dream MCP tools (trigger_dream, get_dream_status).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Request parameters for trigger_dream tool.
#[derive(Debug, Clone, Deserialize)]
pub struct TriggerDreamRequest {
    #[serde(default = "default_blocking")]
    pub blocking: bool,

    #[serde(default)]
    pub dry_run: bool,

    #[serde(default)]
    pub skip_nrem: bool,

    #[serde(default)]
    pub skip_rem: bool,

    #[serde(default = "default_max_duration")]
    #[allow(dead_code)]
    pub max_duration_secs: u64,
}

fn default_blocking() -> bool {
    true
}

fn default_max_duration() -> u64 {
    300
}

impl Default for TriggerDreamRequest {
    fn default() -> Self {
        Self {
            blocking: true,
            dry_run: false,
            skip_nrem: false,
            skip_rem: false,
            max_duration_secs: 300,
        }
    }
}

/// Request parameters for get_dream_status tool.
#[derive(Debug, Deserialize, Default)]
pub struct GetDreamStatusRequest {
    pub dream_id: Option<Uuid>,
}

/// Response from trigger_dream tool.
#[derive(Debug, Clone, Serialize)]
pub struct TriggerDreamResponse {
    pub dream_id: Uuid,
    pub status: DreamStatus,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub nrem_result: Option<NremResult>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub rem_result: Option<RemResult>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<DreamReport>,

    pub dry_run: bool,
}

/// Status of a dream cycle.
#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DreamStatus {
    Queued,
    NremInProgress,
    #[allow(dead_code)]
    NremComplete,
    RemInProgress,
    Completed,
    Interrupted,
    Failed,
    DryRunComplete,
}

impl std::fmt::Display for DreamStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DreamStatus::Queued => write!(f, "queued"),
            DreamStatus::NremInProgress => write!(f, "nrem_in_progress"),
            DreamStatus::NremComplete => write!(f, "nrem_complete"),
            DreamStatus::RemInProgress => write!(f, "rem_in_progress"),
            DreamStatus::Completed => write!(f, "completed"),
            DreamStatus::Interrupted => write!(f, "interrupted"),
            DreamStatus::Failed => write!(f, "failed"),
            DreamStatus::DryRunComplete => write!(f, "dry_run_complete"),
        }
    }
}

/// NREM phase results.
#[derive(Debug, Clone, Serialize)]
pub struct NremResult {
    pub memories_replayed: usize,
    pub edges_strengthened: usize,
    pub edges_weakened: usize,
    pub edges_pruned: usize,
    pub avg_weight_delta: f32,
    pub duration_ms: u64,
    pub completed: bool,
    pub params: HebbianParams,
}

/// Hebbian learning parameters (Constitution dream layer).
#[derive(Debug, Clone, Serialize)]
pub struct HebbianParams {
    pub learning_rate: f32,
    pub weight_decay: f32,
    pub weight_floor: f32,
    pub weight_cap: f32,
    pub recency_bias: f32,
}

impl Default for HebbianParams {
    fn default() -> Self {
        Self {
            learning_rate: 0.01,
            weight_decay: 0.001,
            weight_floor: 0.05,
            weight_cap: 1.0,
            recency_bias: 0.8,
        }
    }
}

/// REM phase results.
#[derive(Debug, Clone, Serialize)]
pub struct RemResult {
    pub queries_generated: usize,
    pub blind_spots_found: usize,
    pub new_edges_created: usize,
    pub avg_semantic_leap: f32,
    pub exploration_coverage: f32,
    pub unique_positions_visited: usize,
    pub duration_ms: u64,
    pub completed: bool,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub blind_spots_sample: Vec<BlindSpotInfo>,
}

/// Information about a discovered blind spot.
#[derive(Debug, Clone, Serialize)]
pub struct BlindSpotInfo {
    pub node_a_id: Uuid,
    pub node_b_id: Uuid,
    pub semantic_distance: f32,
    pub confidence: f32,
}

/// Overall dream cycle report.
#[derive(Debug, Clone, Serialize)]
pub struct DreamReport {
    pub total_duration_ms: u64,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub wake_reason: Option<String>,

    pub pre_entropy: f32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_entropy: Option<f32>,

    pub pre_coherence: f32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_coherence: Option<f32>,

    pub dream_recommended: bool,
    pub recommendations: Vec<String>,
}

/// Response from get_dream_status tool.
#[derive(Debug, Clone, Serialize)]
pub struct GetDreamStatusResponse {
    pub dream_id: Uuid,
    pub status: DreamStatus,
    pub progress_percent: u8,
    pub current_phase: String,
    pub elapsed_ms: u64,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining_ms: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub partial_results: Option<TriggerDreamResponse>,
}

impl From<&context_graph_core::dream::NremReport> for NremResult {
    fn from(report: &context_graph_core::dream::NremReport) -> Self {
        Self {
            memories_replayed: report.memories_replayed,
            edges_strengthened: report.edges_strengthened,
            edges_weakened: report.edges_weakened,
            edges_pruned: report.edges_pruned,
            avg_weight_delta: report.average_weight_delta,
            duration_ms: report.duration.as_millis() as u64,
            completed: report.completed,
            params: HebbianParams::default(),
        }
    }
}

impl From<&context_graph_core::dream::RemReport> for RemResult {
    fn from(report: &context_graph_core::dream::RemReport) -> Self {
        Self {
            queries_generated: report.queries_generated,
            blind_spots_found: report.blind_spots_found,
            new_edges_created: report.new_edges_created,
            avg_semantic_leap: report.average_semantic_leap,
            exploration_coverage: report.exploration_coverage,
            unique_positions_visited: report.unique_nodes_visited,
            duration_ms: report.duration.as_millis() as u64,
            completed: report.completed,
            blind_spots_sample: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trigger_dream_request_defaults() {
        let request = TriggerDreamRequest::default();
        assert!(request.blocking);
        assert!(!request.dry_run);
        assert!(!request.skip_nrem);
        assert!(!request.skip_rem);
        assert_eq!(request.max_duration_secs, 300);
    }

    #[test]
    fn test_dream_status_display() {
        assert_eq!(DreamStatus::Queued.to_string(), "queued");
        assert_eq!(DreamStatus::NremInProgress.to_string(), "nrem_in_progress");
        assert_eq!(DreamStatus::Completed.to_string(), "completed");
        assert_eq!(DreamStatus::DryRunComplete.to_string(), "dry_run_complete");
    }

    #[test]
    fn test_hebbian_params_defaults() {
        let params = HebbianParams::default();
        assert_eq!(params.learning_rate, 0.01);
        assert_eq!(params.weight_decay, 0.001);
        assert_eq!(params.weight_floor, 0.05);
        assert_eq!(params.weight_cap, 1.0);
        assert_eq!(params.recency_bias, 0.8);
    }

    #[test]
    fn test_trigger_dream_response_serialization() {
        let response = TriggerDreamResponse {
            dream_id: Uuid::new_v4(),
            status: DreamStatus::Completed,
            nrem_result: None,
            rem_result: None,
            report: None,
            dry_run: false,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("dream_id"));
        assert!(json.contains("completed"));
    }

    #[test]
    fn test_get_dream_status_response_serialization() {
        let response = GetDreamStatusResponse {
            dream_id: Uuid::new_v4(),
            status: DreamStatus::NremInProgress,
            progress_percent: 45,
            current_phase: "NREM".to_string(),
            elapsed_ms: 81000,
            remaining_ms: Some(99000),
            partial_results: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("nrem_in_progress"));
        assert!(json.contains("45"));
        assert!(json.contains("81000"));
    }
}
