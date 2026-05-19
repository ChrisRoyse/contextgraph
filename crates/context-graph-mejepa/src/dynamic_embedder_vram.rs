use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::dynamic_embedder::{DynamicEmbedderKind, RuntimeEmbedderId, RuntimeRoutingTable};
use crate::error::MejepaInferError;

pub const MEJEPA_DYNAMIC_EMBEDDER_VRAM_BUDGET_EXCEEDED: &str =
    "MEJEPA_DYNAMIC_EMBEDDER_VRAM_BUDGET_EXCEEDED";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DynamicEmbedderArtifactVramEstimate {
    pub id: RuntimeEmbedderId,
    pub kind: DynamicEmbedderKind,
    pub artifact_path: PathBuf,
    pub artifact_bytes: u64,
    pub required_vram_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DynamicEmbedderVramBudget {
    pub static_required_bytes: u64,
    pub budget_bytes: u64,
}

impl DynamicEmbedderVramBudget {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.budget_bytes == 0 {
            return invalid(
                "dynamic_embedder_vram.budget_bytes",
                "budget must be non-zero",
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DynamicEmbedderVramBudgetDecision {
    pub accepted: bool,
    pub reason_code: Option<String>,
    pub static_required_bytes: u64,
    pub current_required_bytes: u64,
    pub candidate_required_bytes: u64,
    pub projected_required_bytes: u64,
    pub budget_bytes: u64,
    pub active_dynamic_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DynamicEmbedderUtilityScore {
    pub id: RuntimeEmbedderId,
    pub last_used_unix_ms: i64,
    pub mean_per_cell_contribution: f32,
}

impl DynamicEmbedderUtilityScore {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        self.id.validate().map_err(embed_error)?;
        if !self.id.is_dynamic() {
            return invalid(
                "dynamic_embedder_vram.utility.id",
                "utility rows must reference EDynamic ids",
            );
        }
        if self.last_used_unix_ms < 0 {
            return invalid(
                "dynamic_embedder_vram.utility.last_used_unix_ms",
                "last_used_unix_ms must be non-negative",
            );
        }
        if !self.mean_per_cell_contribution.is_finite() || self.mean_per_cell_contribution < 0.0 {
            return invalid(
                "dynamic_embedder_vram.utility.mean_per_cell_contribution",
                "contribution must be finite and non-negative",
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DynamicEmbedderEvictionDecision {
    pub eviction_required: bool,
    pub budget_satisfied: bool,
    pub reason_code: Option<String>,
    pub before_required_bytes: u64,
    pub after_required_bytes: u64,
    pub budget_bytes: u64,
    pub freed_vram_bytes: u64,
    pub evicted_ids: Vec<RuntimeEmbedderId>,
    pub retained_active_ids: Vec<RuntimeEmbedderId>,
    pub missing_utility_ids: Vec<RuntimeEmbedderId>,
}

pub fn estimate_dynamic_embedder_artifact_vram(
    id: RuntimeEmbedderId,
    kind: DynamicEmbedderKind,
    artifact_path: impl AsRef<Path>,
) -> Result<DynamicEmbedderArtifactVramEstimate, MejepaInferError> {
    id.validate().map_err(embed_error)?;
    if !id.is_dynamic() {
        return invalid(
            "dynamic_embedder_vram.id",
            "VRAM estimates are only for EDynamic candidates",
        );
    }
    let artifact_path = artifact_path.as_ref().to_path_buf();
    let metadata = std::fs::metadata(&artifact_path)
        .map_err(|source| MejepaInferError::io("metadata", &artifact_path, source))?;
    if !metadata.is_file() {
        return invalid(
            "dynamic_embedder_vram.artifact_path",
            format!("{} is not a file", artifact_path.display()),
        );
    }
    let artifact_bytes = metadata.len();
    if artifact_bytes == 0 {
        return invalid(
            "dynamic_embedder_vram.artifact_bytes",
            "artifact file is empty",
        );
    }
    Ok(DynamicEmbedderArtifactVramEstimate {
        id,
        kind,
        artifact_path,
        artifact_bytes,
        required_vram_bytes: artifact_bytes,
    })
}

pub fn check_dynamic_embedder_promotion_vram_budget(
    table: &RuntimeRoutingTable,
    budget: DynamicEmbedderVramBudget,
    candidate_required_bytes: u64,
) -> Result<DynamicEmbedderVramBudgetDecision, MejepaInferError> {
    budget.validate()?;
    if candidate_required_bytes == 0 {
        return invalid(
            "dynamic_embedder_vram.candidate_required_bytes",
            "candidate VRAM requirement must be non-zero",
        );
    }
    let current_required_bytes = table
        .required_vram_bytes(budget.static_required_bytes)
        .map_err(embed_error)?;
    let projected_required_bytes = current_required_bytes
        .checked_add(candidate_required_bytes)
        .ok_or_else(|| MejepaInferError::InvalidInput {
            field: "dynamic_embedder_vram.projected_required_bytes".to_string(),
            detail: "projected VRAM requirement overflowed u64".to_string(),
        })?;
    let accepted = projected_required_bytes <= budget.budget_bytes;
    Ok(DynamicEmbedderVramBudgetDecision {
        accepted,
        reason_code: (!accepted).then(|| MEJEPA_DYNAMIC_EMBEDDER_VRAM_BUDGET_EXCEEDED.to_string()),
        static_required_bytes: budget.static_required_bytes,
        current_required_bytes,
        candidate_required_bytes,
        projected_required_bytes,
        budget_bytes: budget.budget_bytes,
        active_dynamic_count: table.active_dynamic_count(),
    })
}

pub fn plan_dynamic_embedder_evictions(
    table: &RuntimeRoutingTable,
    budget: DynamicEmbedderVramBudget,
    utilities: &[DynamicEmbedderUtilityScore],
) -> Result<DynamicEmbedderEvictionDecision, MejepaInferError> {
    budget.validate()?;
    let mut utility_by_id = BTreeMap::new();
    for utility in utilities {
        utility.validate()?;
        utility_by_id.insert(utility.id.clone(), utility.clone());
    }
    let before_required_bytes = table
        .required_vram_bytes(budget.static_required_bytes)
        .map_err(embed_error)?;
    if before_required_bytes <= budget.budget_bytes {
        return Ok(DynamicEmbedderEvictionDecision {
            eviction_required: false,
            budget_satisfied: true,
            reason_code: None,
            before_required_bytes,
            after_required_bytes: before_required_bytes,
            budget_bytes: budget.budget_bytes,
            freed_vram_bytes: 0,
            evicted_ids: Vec::new(),
            retained_active_ids: active_dynamic_ids(table, &BTreeSet::new()),
            missing_utility_ids: Vec::new(),
        });
    }

    let mut candidates = table
        .dynamic_records
        .iter()
        .filter(|record| record.active)
        .map(|record| {
            let utility = utility_by_id.get(&record.id).cloned();
            EvictionCandidate {
                id: record.id.clone(),
                required_vram_bytes: record.required_vram_bytes,
                last_used_unix_ms: utility
                    .as_ref()
                    .map(|item| item.last_used_unix_ms)
                    .unwrap_or(0),
                mean_per_cell_contribution: utility
                    .as_ref()
                    .map(|item| item.mean_per_cell_contribution)
                    .unwrap_or(0.0),
                missing_utility: utility.is_none(),
            }
        })
        .collect::<Vec<_>>();
    candidates.sort_by(compare_eviction_candidate);

    let mut after_required_bytes = before_required_bytes;
    let mut freed_vram_bytes = 0u64;
    let mut evicted_ids = Vec::new();
    let mut missing_utility_ids = BTreeSet::new();
    for candidate in candidates {
        if after_required_bytes <= budget.budget_bytes {
            break;
        }
        after_required_bytes = after_required_bytes.saturating_sub(candidate.required_vram_bytes);
        freed_vram_bytes = freed_vram_bytes
            .checked_add(candidate.required_vram_bytes)
            .ok_or_else(|| MejepaInferError::InvalidInput {
                field: "dynamic_embedder_vram.freed_vram_bytes".to_string(),
                detail: "freed VRAM overflowed u64".to_string(),
            })?;
        if candidate.missing_utility {
            missing_utility_ids.insert(candidate.id.clone());
        }
        evicted_ids.push(candidate.id);
    }
    let evicted_set = evicted_ids.iter().cloned().collect::<BTreeSet<_>>();
    let budget_satisfied = after_required_bytes <= budget.budget_bytes;
    Ok(DynamicEmbedderEvictionDecision {
        eviction_required: true,
        budget_satisfied,
        reason_code: (!budget_satisfied)
            .then(|| MEJEPA_DYNAMIC_EMBEDDER_VRAM_BUDGET_EXCEEDED.to_string()),
        before_required_bytes,
        after_required_bytes,
        budget_bytes: budget.budget_bytes,
        freed_vram_bytes,
        evicted_ids,
        retained_active_ids: active_dynamic_ids(table, &evicted_set),
        missing_utility_ids: missing_utility_ids.into_iter().collect(),
    })
}

#[derive(Debug, Clone)]
struct EvictionCandidate {
    id: RuntimeEmbedderId,
    required_vram_bytes: u64,
    last_used_unix_ms: i64,
    mean_per_cell_contribution: f32,
    missing_utility: bool,
}

fn compare_eviction_candidate(left: &EvictionCandidate, right: &EvictionCandidate) -> Ordering {
    left.mean_per_cell_contribution
        .partial_cmp(&right.mean_per_cell_contribution)
        .unwrap_or(Ordering::Equal)
        .then_with(|| left.last_used_unix_ms.cmp(&right.last_used_unix_ms))
        .then_with(|| left.id.cmp(&right.id))
}

fn active_dynamic_ids(
    table: &RuntimeRoutingTable,
    evicted: &BTreeSet<RuntimeEmbedderId>,
) -> Vec<RuntimeEmbedderId> {
    table
        .dynamic_records
        .iter()
        .filter(|record| record.active && !evicted.contains(&record.id))
        .map(|record| record.id.clone())
        .collect()
}

fn embed_error(err: context_graph_mejepa_embedders::EmbedError) -> MejepaInferError {
    MejepaInferError::InvalidInput {
        field: "dynamic_embedder_vram".to_string(),
        detail: err.to_string(),
    }
}

fn invalid<T>(field: &str, detail: impl Into<String>) -> Result<T, MejepaInferError> {
    Err(MejepaInferError::InvalidInput {
        field: field.to_string(),
        detail: detail.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dynamic_embedder::{DynamicEmbedderRegistryRecord, RuntimeRoutingTable};

    #[test]
    fn promotion_budget_accepts_fit_and_rejects_overflow() {
        let table = RuntimeRoutingTable::new(2, vec![record(1, "existing", 256)]).unwrap();
        let budget = DynamicEmbedderVramBudget {
            static_required_bytes: 512,
            budget_bytes: 1024,
        };
        let fit = check_dynamic_embedder_promotion_vram_budget(&table, budget, 128).unwrap();
        let exceed = check_dynamic_embedder_promotion_vram_budget(&table, budget, 512).unwrap();

        assert!(fit.accepted);
        assert_eq!(fit.projected_required_bytes, 896);
        assert!(!exceed.accepted);
        assert_eq!(
            exceed.reason_code.as_deref(),
            Some(MEJEPA_DYNAMIC_EMBEDDER_VRAM_BUDGET_EXCEEDED)
        );
    }

    #[test]
    fn eviction_prefers_low_contribution_then_lru() {
        let table = RuntimeRoutingTable::new(
            2,
            vec![
                record(1, "low_lru", 400),
                record(2, "low_contrib", 300),
                record(3, "high_value", 200),
            ],
        )
        .unwrap();
        let decision = plan_dynamic_embedder_evictions(
            &table,
            DynamicEmbedderVramBudget {
                static_required_bytes: 500,
                budget_bytes: 850,
            },
            &[
                utility(1, "low_lru", 50, 0.02),
                utility(2, "low_contrib", 100, 0.01),
                utility(3, "high_value", 200, 0.50),
            ],
        )
        .unwrap();

        assert!(decision.eviction_required);
        assert!(decision.budget_satisfied);
        assert_eq!(
            decision.evicted_ids,
            vec![
                RuntimeEmbedderId::dynamic(2, "low_contrib").unwrap(),
                RuntimeEmbedderId::dynamic(1, "low_lru").unwrap()
            ]
        );
        assert_eq!(decision.after_required_bytes, 700);
    }

    fn record(
        sequence: u32,
        name: &str,
        required_vram_bytes: u64,
    ) -> DynamicEmbedderRegistryRecord {
        DynamicEmbedderRegistryRecord {
            id: RuntimeEmbedderId::dynamic(sequence, name).unwrap(),
            registry_version: 2,
            kind: DynamicEmbedderKind::LearnedHead,
            dimension: 128,
            route_languages: vec!["python".to_string()],
            route_entity_types: vec!["*".to_string()],
            forward_artifact_path: format!(
                "/zfs/archive/contextgraph/models/dynamic/edynamic_{sequence}_{name}/forward.bin"
            ),
            forward_artifact_sha256:
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            required_vram_bytes,
            active: true,
            promoted_at_unix_ms: 1_779_100_000_000,
        }
    }

    fn utility(
        sequence: u32,
        name: &str,
        last_used_unix_ms: i64,
        mean_per_cell_contribution: f32,
    ) -> DynamicEmbedderUtilityScore {
        DynamicEmbedderUtilityScore {
            id: RuntimeEmbedderId::dynamic(sequence, name).unwrap(),
            last_used_unix_ms,
            mean_per_cell_contribution,
        }
    }
}
